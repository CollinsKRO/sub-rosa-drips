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
