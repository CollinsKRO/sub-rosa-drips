// Copyright (c) 2026 Sub Rosa contributors
import {
  ROUND_PHASE_LABELS,
  type RoundPhase,
  roundPhaseLabel,
} from "@sub-rosa/sdk";
import type { RoundStatus } from "../dashboard/types";

// Re-export the shared phase vocabulary so the UI cannot invent a fourth phase
// name that drifts from `packages/sdk/src/round-status.ts`.
export { ROUND_PHASE_LABELS, roundPhaseLabel };
export type { RoundPhase };

export interface ClassifyRoundPhaseInput {
  status: RoundStatus;
  drandPublished: boolean;
}

/** Status tags the shared phase helper accepts; anything else is sealed. */
const KNOWN_ROUND_STATUSES: readonly RoundStatus[] = [
  "Open",
  "Revealing",
  "Cleared",
  "Settled",
  "Voided",
];

export function classifyRoundPhase({
  status,
  drandPublished,
}: ClassifyRoundPhaseInput): RoundPhase {
  if (status === "Settled" || status === "Voided") return "Settled";
  if (status === "Revealing" || status === "Cleared" || drandPublished) {
    return "Reveal";
  }
  return "Open";
}

/** True once the shared phase helper says bid values are public. */
export function isRevealPhase(phase: RoundPhase): boolean {
  return phase !== "Open";
}

/**
 * Normalize an on-chain or recorded status tag into a `RoundStatus` the shared
 * phase helper accepts. Unknown tags classify conservatively as "Open" so a
 * sealed value can never be formatted from an unrecognized status.
 */
export function roundStatusFromTag(
  tag: string | null | undefined,
): RoundStatus {
  return (KNOWN_ROUND_STATUSES as readonly string[]).includes(tag ?? "")
    ? (tag as RoundStatus)
    : "Open";
}
