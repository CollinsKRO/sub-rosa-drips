// Copyright (c) 2026 Sub Rosa contributors
import type { RoundStatus as DashboardRoundStatus } from "../dashboard/types";
import type { RoundStatus as SdkRoundStatus } from "@sub-rosa/sdk";

export type RoundPhase = "Open" | "Reveal" | "Settled";

export interface ClassifyRoundPhaseInput {
  status: DashboardRoundStatus;
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

// ---------------------------------------------------------------------------
// Lifecycle step derivation — driven by the SDK RoundStatus only.
// ---------------------------------------------------------------------------

export type LifecycleStep = "commit" | "reveal" | "settle";

export interface LifecycleStepResult {
  /** The active lifecycle step, or null when the status is an error state. */
  step: LifecycleStep | null;
  /** True when the status indicates an unresolvable or unknown round. */
  error: boolean;
  /** True when the settle step should be disabled (round not yet settled). */
  settleDisabled: boolean;
}

/**
 * Map an SDK `RoundStatus` to the active lifecycle step.
 *
 * - Open            → commit active
 * - Revealing       → reveal active
 * - Cleared         → reveal active (cleared but not yet settled)
 * - Settled         → settle done
 * - Voided          → settle done (voided is terminal like settled)
 * - Unknown/NotFound → error state, no step
 */
export function lifecycleStepFromStatus(
  status: SdkRoundStatus,
): LifecycleStepResult {
  switch (status) {
    case "Open":
      return { step: "commit", error: false, settleDisabled: true };
    case "Revealing":
    case "Cleared":
      return { step: "reveal", error: false, settleDisabled: true };
    case "Settled":
    case "Voided":
      return { step: "settle", error: false, settleDisabled: false };
    case "Unknown":
    case "NotFound":
      return { step: null, error: true, settleDisabled: true };
  }
}
