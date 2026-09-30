import { normalizeError } from "@sub-rosa/logging/errors";
// Copyright (c) 2026 Sub Rosa contributors
import { createLogger } from '@sub-rosa/logging';
const diagnostics = createLogger("services.keeper.src.queue");
import { KeeperStore, normalizeRoundId, parseLeaseMs } from "./store.js";

function usage() {
  diagnostics.info("usage-npm-run-queue-command-args-commands-add-roundid-a", `
Usage: npm run queue <command> [args]

Commands:
  add <roundId>      Add a round to the watched queue
  list               List all watched rounds and their status
  remove <roundId>   Remove a round from the queue
  claim <roundId>    Take the exclusive lease on a round (owner: KEEPER_OWNER)
  release <roundId>  Give back a lease this owner holds on a round
`);
  process.exit(1);
}

function main() {
  const args = process.argv.slice(2);
  if (args.length === 0) {
    usage();
  }

  const cmd = args[0];
  const store = new KeeperStore();
  const contractId = process.env.ROUND_CONTRACT_ID;
  const network = process.env.NETWORK_PASSPHRASE;
  const owner = process.env.KEEPER_OWNER?.trim() || `queue-cli-${process.pid}`;

  if (cmd === "add") {
    const rawRoundId = args[1];
    if (!rawRoundId) {
      diagnostics.error("error-missing-roundid", "Error: missing roundId");
      usage();
    }
    const roundId = normalizeRoundId(rawRoundId);
    store.addRound(roundId, { contractId, network });
    diagnostics.info("added-round", `Added round ${roundId} to the queue.`);
  } else if (cmd === "list") {
    const rounds = store.listRounds();
    if (rounds.length === 0) {
      diagnostics.info("queue-is-empty", "Queue is empty.");
      return;
    }
    diagnostics.info("watching", `Watching ${rounds.length} rounds:\n`);
    for (const r of rounds) {
      const extra = r.lastAction ? ` (action: ${r.lastAction})` : "";
      const err = r.lastError ? ` (error: ${r.lastError})` : "";
      const contract = r.contractId ? ` [${r.contractId}]` : "";
      const lease = store.getLease(r.roundId);
      const leased = lease ? ` [lease: ${lease.owner} until ${lease.expiresAtMs}]` : "";
      diagnostics.info("round", `- Round ${r.roundId}${contract}: ${r.lastStatus}${extra}${err} [retries: ${r.retryCount}]${leased}`);
    }
  } else if (cmd === "remove") {
    const rawRoundId = args[1];
    if (!rawRoundId) {
      diagnostics.error("error-missing-roundid-2", "Error: missing roundId");
      usage();
    }
    const roundId = normalizeRoundId(rawRoundId);
    store.removeRound(roundId);
    diagnostics.info("removed-round", `Removed round ${roundId} from the queue.`);
  } else if (cmd === "claim") {
    const rawRoundId = args[1];
    if (!rawRoundId) {
      diagnostics.error("error-missing-roundid-3", "Error: missing roundId");
      usage();
    }
    const roundId = normalizeRoundId(rawRoundId);
    const leaseMs = parseLeaseMs(process.env.KEEPER_LEASE_MS);
    const claim = store.claimRound(roundId, {
      owner,
      contractId,
      network,
      ...(leaseMs !== undefined ? { leaseMs } : {}),
    });
    if (claim.claimed) {
      diagnostics.info("claimed-round-lease", `Claimed round ${roundId} as ${owner} until ${claim.lease.expiresAtMs}.`);
    } else {
      diagnostics.error(
        "round-lease-held",
        `Round ${roundId} is leased by ${claim.lease.owner} until ${claim.lease.expiresAtMs}.`,
      );
      process.exitCode = 1;
    }
  } else if (cmd === "release") {
    const rawRoundId = args[1];
    if (!rawRoundId) {
      diagnostics.error("error-missing-roundid-4", "Error: missing roundId");
      usage();
    }
    const roundId = normalizeRoundId(rawRoundId);
    if (store.releaseLease(roundId, { owner, contractId, network })) {
      diagnostics.info("released-round-lease", `Released the ${roundId} lease held by ${owner}.`);
    } else {
      diagnostics.error("round-lease-not-owned", `Round ${roundId} has no lease held by ${owner}.`);
      process.exitCode = 1;
    }
  } else {
    diagnostics.error("unknown-command", `Unknown command: ${cmd}`);
    usage();
  }
}

try {
  main();
} catch (error) {
  diagnostics.error("error", `Error: ${normalizeError(error).message}`);
  process.exit(1);
}
