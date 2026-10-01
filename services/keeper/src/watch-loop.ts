import { normalizeError } from "@sub-rosa/logging/errors";
// Copyright (c) 2026 Sub Rosa contributors
// Shared watch loop. Keeps in-flight rounds moving through
// void-if-stale → keep → close, persisting status into the KeeperStore and
// emitting lightweight logs. Independent of how the loop is started
// (one-shot CLI `watch.ts`, or combined with the status HTTP API in `serve.ts`).
//
// The keeper process can run the status API *and* the watch loop in the same
// process: both read from the same on-chain source and the same persisted
// store. The status API never writes state; only the watch loop advances
// rounds on-chain.

import type { SubRosaClient } from "@sub-rosa/sdk";
import type { DrandClient } from "@sub-rosa/tlock";
import { resolveTimeContext, systemTime, type PartialTimeContext, type Scheduler } from "@sub-rosa/time";

import {
  discoverRoundIds,
  parseRoundIdSpec,
  type KeeperDeps,
  type WatchTickResult,
  watchRound,
} from "./keeper.js";
import type { SettlementGuard } from "./settlement-guard.js";
import type { KeeperLogger } from "./keeper.js";
import { KeeperStore } from "./store.js";
import { KeeperQueue } from "./queue.js";

export interface RunWatchLoopParams {
  sdk: SubRosaClient;
  drand: DrandClient;
  log: KeeperLogger;
  pollMs: number;
  contractId: string;
  network: string;
  store: KeeperStore;
  settlementGuard: SettlementGuard;
  isStopping: () => boolean;
  /** Injectable wall clock and scheduler. Default: systemTime. */
  time?: PartialTimeContext;
  queue?: KeeperQueue;
  shutdownTimeoutMs?: number;
}

const bigintReplacer = (_k: string, v: unknown): unknown =>
  typeof v === "bigint" ? v.toString() : v;

function summarizeTick(t: WatchTickResult): string {
  const parts: string[] = [t.finalStatus];
  if (t.void?.voided) parts.push("voided");
  if (t.keep?.openedReveal) parts.push("opened");
  if (t.keep?.revealed.length) parts.push(`revealed×${t.keep.revealed.length}`);
  if (t.close?.cleared) parts.push("cleared");
  if (t.close?.settled) parts.push("settled");
  return parts.join(", ");
}

async function resolveRoundIds(reader: SubRosaClient): Promise<bigint[]> {
  const spec = process.env.WATCH_ROUND_IDS?.trim();
  if (spec) return parseRoundIdSpec(spec);
  const single = process.env.ROUND_ID?.trim();
  if (single) return [BigInt(single)];
  return discoverRoundIds(reader, {
    from: BigInt(process.env.WATCH_FROM ?? "1"),
    maxProbe: Number(process.env.WATCH_MAX_ROUNDS ?? "64"),
  });
}

/**
 * Bounds in-flight round execution during shutdown by the given scheduler and timeout.
 */
async function waitForInFlight<T>(
  promise: Promise<T>,
  opts: {
    isStopping: () => boolean;
    scheduler: Scheduler;
    timeoutMs: number;
    roundId: bigint;
  },
): Promise<T> {
  const { isStopping, scheduler, timeoutMs, roundId } = opts;
  let done = false;

  const timeoutPromise = new Promise<never>((_, reject) => {
    const triggerTimeout = () => {
      scheduler.setTimeout(() => {
        if (!done) {
          reject(new Error(`Shutdown timeout (${timeoutMs}ms) waiting for round ${roundId}`));
        }
      }, timeoutMs);
    };

    if (isStopping()) {
      triggerTimeout();
      return;
    }

    const checkInterval = 25;
    let pollHandle: ReturnType<typeof scheduler.setTimeout> | undefined;

    const check = () => {
      if (done) return;
      if (isStopping()) {
        triggerTimeout();
        return;
      }
      pollHandle = scheduler.setTimeout(check, checkInterval);
    };

    pollHandle = scheduler.setTimeout(check, checkInterval);
  });

  return Promise.race([
    promise.finally(() => {
      done = true;
    }),
    timeoutPromise,
  ]);
}

export async function runWatchLoop(params: RunWatchLoopParams): Promise<void> {
  const {
    sdk,
    drand,
    log,
    pollMs,
    contractId,
    network,
    store,
    settlementGuard,
    isStopping,
    time,
    owner: explicitOwner,
    leaseMs,
  } = params;

  // Load the checkpoint / stored rounds and validate before claiming work
  const storedRounds = store.listRounds();
  validateStoredCheckpoint(storedRounds, { contractId, network });

  const resolvedTime = resolveTimeContext(systemTime, time);
  const { clock, scheduler } = resolvedTime;
  const deps: KeeperDeps = { sdk, drand, log, time: resolvedTime, settlementGuard };
  const queue = params.queue ?? new KeeperQueue(store, { contractId, network });
  const shutdownTimeoutMs = params.shutdownTimeoutMs ?? 30000;

  const shouldStop = () => isStopping() || queue.isStopping();

  while (!shouldStop()) {
    const started = clock.nowMs();
    let discoveredIds: bigint[] = [];
    try {
      discoveredIds = await resolveRoundIds(sdk);
      for (const id of discoveredIds) {
        queue.enqueue(id, { contractId, network });
      }
    } catch (e) {
      log(`watch: failed to list/discover rounds: ${normalizeError(e).message}`);
    }

    queue.syncWithStore();

    if (queue.size() === 0 && queue.inFlightCount() === 0) {
      log("no active rounds found in queue — waiting");
    }

    const batchSize = queue.size();
    for (let i = 0; i < batchSize; i++) {
      if (shouldStop()) break;

      const storedRound = queue.claim();
      if (!storedRound) break;

      const roundId = BigInt(storedRound.roundId);
      try {
        const canSettleCheck = settlementGuard.canSettle(roundId);
        if (!canSettleCheck.allowed) {
          // Settlement already in-flight or terminal; close phase handled by guard
        }

        const tickPromise = watchRound(deps, roundId);
        const tick = await waitForInFlight(tickPromise, {
          isStopping: shouldStop,
          scheduler,
          timeoutMs: shutdownTimeoutMs,
          roundId,
        });

        const active =
          tick.finalStatus !== "Settled" && tick.finalStatus !== "Voided";
        const acted =
          tick.void?.voided ||
          tick.keep?.openedReveal ||
          (tick.keep?.revealed.length ?? 0) > 0 ||
          tick.close?.cleared ||
          tick.close?.settled;

        if (tick.close?.settled) {
          settlementGuard.markTerminal(roundId, "settled on-chain");
        } else if (tick.close?.voided || tick.finalStatus === "Voided") {
          settlementGuard.markTerminal(roundId, "voided on-chain");
        }

        queue.complete(roundId, {
          lastStatus: tick.finalStatus,
          retryCount: 0,
          lastError: undefined,
          lastAction: acted ? summarizeTick(tick) : storedRound.lastAction,
        });

        if (!active) {
          // Terminal success: the step is done, so the round goes back to the
          // queue instead of waiting out the lease.
          store.releaseLease(roundId, { owner, contractId, network });
        }

        if (active || acted) {
          log(
            `[round ${roundId}] ${summarizeTick(tick)}` +
              (acted ? " " + JSON.stringify(tick, bigintReplacer) : ""),
          );
        }
      } catch (e) {
        log(`[round ${roundId}] tick failed: ${normalizeError(e).message}`);
        settlementGuard.markRetryable(
          roundId,
          normalizeError(e).message,
        );
        const stored = store.getRound(roundId);
        queue.release(roundId, {
          retryCount: (stored?.retryCount ?? 0) + 1,
          lastError: normalizeError(e).message,
        });
        if (isDefinitiveContractFailure(e)) {
          store.releaseLease(roundId, { owner, contractId, network });
          log(`[round ${roundId}] lease released after definitive contract failure`);
        } else {
          log(
            `[round ${roundId}] lease kept until ` +
              `${clock.toISOString(claim.lease.expiresAtMs)} — ${normalizeError(e).message}`,
          );
        }
      }
    }

    if (shouldStop()) break;
    const elapsed = clock.nowMs() - started;
    const wait = Math.max(0, pollMs - elapsed);
    if (wait > 0) {
      const step = Math.min(wait, 250);
      let waited = 0;
      while (waited < wait && !shouldStop()) {
        const toSleep = Math.min(step, wait - waited);
        await scheduler.sleep(toSleep);
        waited += toSleep;
      }
    }
  }
}
