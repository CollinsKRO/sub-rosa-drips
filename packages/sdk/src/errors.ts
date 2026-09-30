// SPDX-License-Identifier: MIT
import type {
  EscrowConservationPhase,
  EscrowConservationReport,
} from "./conservation.js";

export class SubRosaClientConfigError extends Error {
  readonly name = "SubRosaClientConfigError";

  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
  }
}

export interface NetworkMismatchErrorParams {
  contractId: string;
  configuredPassphrase: string;
  rpcPassphrase: string;
  rpcUrl: string;
  reason: "passphrase" | "contract_not_found";
}

/** Raised before contract simulation/signing when network configuration conflicts. */
export class SubRosaNetworkMismatchError extends Error {
  readonly name = "SubRosaNetworkMismatchError";
  readonly contractId: string;
  readonly configuredPassphrase: string;
  readonly rpcPassphrase: string;
  readonly rpcUrl: string;
  readonly reason: NetworkMismatchErrorParams["reason"];

  constructor(params: NetworkMismatchErrorParams) {
    const message =
      params.reason === "passphrase"
        ? `networkPassphrase ${JSON.stringify(params.configuredPassphrase)} does not match RPC network ${JSON.stringify(params.rpcPassphrase)} at ${params.rpcUrl}; use the passphrase and contract ID from the same deployment`
        : `contract ${params.contractId} was not found on RPC network ${JSON.stringify(params.rpcPassphrase)} at ${params.rpcUrl}; check that contractId and networkPassphrase refer to the same deployment`;
    super(message);
    this.contractId = params.contractId;
    this.configuredPassphrase = params.configuredPassphrase;
    this.rpcPassphrase = params.rpcPassphrase;
    this.rpcUrl = params.rpcUrl;
    this.reason = params.reason;
  }
}

export class SubRosaSubmitError extends Error {
  readonly name = "SubRosaSubmitError";

  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
  }
}

export class SubRosaTransactionError extends Error {
  readonly name = "SubRosaTransactionError";
  readonly hash: string;
  readonly status: string;

  constructor(hash: string, status: string, options?: ErrorOptions) {
    super(`transaction ${hash} ended with status ${status}`, options);
    this.hash = hash;
    this.status = status;
  }
}

export class SubRosaMissingReturnValueError extends Error {
  readonly name = "SubRosaMissingReturnValueError";
  readonly hash: string;

  constructor(hash: string) {
    super(`transaction ${hash} succeeded without a return value`);
    this.hash = hash;
  }
}

export interface TimeoutErrorParams {
  hash: string;
  submitter: string;
  lastStatus: string;
  timeoutMs: number;
  pollIntervalMs: number;
}

export type PreflightFailureKind =
  | "rpc_error"
  | "simulation_error"
  | "expired_state"
  | "contract_error"
  | "malformed_response"
  | "escrow_not_conserved";

export interface SubRosaPreflightErrorParams {
  kind: PreflightFailureKind;
  operation: string;
  message: string;
  simulationError?: string;
  contractErrorCode?: number;
  contractErrorMessage?: string;
  restoreMinResourceFee?: bigint;
  cause?: unknown;
}

/** Typed error for preflight/simulation failures before transaction submission. */
export class SubRosaPreflightError extends Error {
  readonly name = "SubRosaPreflightError";
  readonly kind: PreflightFailureKind;
  readonly operation: string;
  readonly simulationError?: string;
  readonly contractErrorCode?: number;
  readonly contractErrorMessage?: string;
  readonly restoreMinResourceFee?: bigint;

  constructor(params: SubRosaPreflightErrorParams) {
    super(params.message, params.cause ? { cause: params.cause } : undefined);
    this.kind = params.kind;
    this.operation = params.operation;
    this.simulationError = params.simulationError;
    this.contractErrorCode = params.contractErrorCode;
    this.contractErrorMessage = params.contractErrorMessage;
    this.restoreMinResourceFee = params.restoreMinResourceFee;
  }
}

export interface EscrowConservationErrorParams {
  roundId: bigint;
  /** The operation whose escrow accounting failed to balance. */
  phase: EscrowConservationPhase;
  report: EscrowConservationReport;
  cause?: unknown;
}

/**
 * Typed error for a round whose escrow does not reconcile before payout.
 *
 * The Round contract refuses to `settle` or `void` such a round with
 * `EscrowNotConserved`; this error is the off-chain equivalent, raised before
 * any transaction is built, so a keeper can halt instead of burning a fee.
 */
export class SubRosaEscrowConservationError extends SubRosaPreflightError {
  readonly roundId: bigint;
  readonly phase: EscrowConservationPhase;
  readonly report: EscrowConservationReport;

  constructor(params: EscrowConservationErrorParams) {
    const stranded = params.report.stranded;
    const summary = params.report.issues
      .slice(0, 3)
      .map((i) => i.message)
      .join("; ");
    super({
      kind: "escrow_not_conserved",
      operation: params.phase,
      message:
        `round ${params.roundId} does not conserve escrow before ${params.phase}: ` +
        `held ${params.report.escrowHeld}, pays ${params.report.payable}, ` +
        `refunds ${params.report.refundable}, strands ${stranded}` +
        (summary ? ` (${summary})` : ""),
      cause: params.cause,
    });
    this.roundId = params.roundId;
    this.phase = params.phase;
    this.report = params.report;
  }
}

export class SubRosaTimeoutError extends Error {
  readonly name = "SubRosaTimeoutError";
  readonly hash: string;
  readonly submitter: string;
  readonly lastStatus: string;
  readonly timeoutMs: number;
  readonly pollIntervalMs: number;

  constructor(params: TimeoutErrorParams) {
    super(
      `${params.submitter} submitted ${params.hash}, but RPC did not finalize it in time (last=${params.lastStatus})`,
    );
    this.hash = params.hash;
    this.submitter = params.submitter;
    this.lastStatus = params.lastStatus;
    this.timeoutMs = params.timeoutMs;
    this.pollIntervalMs = params.pollIntervalMs;
  }
}
