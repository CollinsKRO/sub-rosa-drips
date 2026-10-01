// SPDX-License-Identifier: MIT

export type RoundContractErrorCode =
  | "NotInitialized"
  | "AlreadyInitialized"
  | "RoundNotFound"
  | "BidNotFound"
  | "CommitClosed"
  | "CommitNotClosed"
  | "CommitDeadlineAfterReveal"
  | "RevealNotOpen"
  | "RevealAlreadyOpen"
  | "RevealWindowClosed"
  | "RevealStillOpen"
  | "NotCleared"
  | "AlreadyCleared"
  | "AlreadySettled"
  | "RoundVoided"
  | "NotVoidable"
  | "WrongStatus"
  | "InvalidDrandSignature"
  | "HashMismatch"
  | "AlreadyRevealed"
  | "PayloadTooLarge"
  | "InvalidAmount"
  | "BidExceedsEscrow"
  | "DeadlineInPast"
  | "NoValidBids"
  | "RoundFull"
  | "InvalidLimit";

export interface RoundContractErrorSpec {
  readonly code: number;
  readonly name: RoundContractErrorCode;
  readonly retryable: boolean;
}

export interface ContractErrorEntry {
  readonly code: number;
  readonly name: string;
}

/**
 * Complete, single-source mapping of Soroban round contract errors to stable SDK codes and retryable flags.
 * Derived from contracts/round/src/error_paths.rs, contracts/round/src/types.rs, and ERRORS.md.
 */
export const ROUND_CONTRACT_ERRORS: Readonly<Record<number, RoundContractErrorSpec>> = Object.freeze({
  1: { code: 1, name: "NotInitialized", retryable: false },
  2: { code: 2, name: "AlreadyInitialized", retryable: false },
  3: { code: 3, name: "RoundNotFound", retryable: false },
  4: { code: 4, name: "BidNotFound", retryable: false },
  10: { code: 10, name: "CommitClosed", retryable: false },
  11: { code: 11, name: "CommitNotClosed", retryable: true },
  12: { code: 12, name: "CommitDeadlineAfterReveal", retryable: false },
  13: { code: 13, name: "RevealNotOpen", retryable: true },
  14: { code: 14, name: "RevealAlreadyOpen", retryable: false },
  15: { code: 15, name: "RevealWindowClosed", retryable: false },
  16: { code: 16, name: "RevealStillOpen", retryable: true },
  17: { code: 17, name: "NotCleared", retryable: true },
  18: { code: 18, name: "AlreadyCleared", retryable: false },
  19: { code: 19, name: "AlreadySettled", retryable: false },
  20: { code: 20, name: "RoundVoided", retryable: false },
  21: { code: 21, name: "NotVoidable", retryable: true },
  22: { code: 22, name: "WrongStatus", retryable: false },
  30: { code: 30, name: "InvalidDrandSignature", retryable: false },
  31: { code: 31, name: "HashMismatch", retryable: false },
  32: { code: 32, name: "AlreadyRevealed", retryable: false },
  33: { code: 33, name: "PayloadTooLarge", retryable: false },
  34: { code: 34, name: "InvalidAmount", retryable: false },
  35: { code: 35, name: "BidExceedsEscrow", retryable: false },
  36: { code: 36, name: "DeadlineInPast", retryable: false },
  37: { code: 37, name: "NoValidBids", retryable: false },
  38: { code: 38, name: "RoundFull", retryable: false },
  39: { code: 39, name: "InvalidLimit", retryable: false },
});

export const ROUND_CONTRACT_ERRORS_BY_NAME: Readonly<Record<string, RoundContractErrorSpec>> = Object.freeze(
  Object.fromEntries(
    Object.values(ROUND_CONTRACT_ERRORS).map((spec) => [spec.name, spec]),
  ),
);

/**
 * Look up a contract error specification by numeric code or variant name.
 */
export function getRoundContractError(
  codeOrName: number | string | undefined | null,
): RoundContractErrorSpec | undefined {
  if (codeOrName === undefined || codeOrName === null) return undefined;
  if (typeof codeOrName === "number") {
    return ROUND_CONTRACT_ERRORS[codeOrName];
  }
  const numeric = Number(codeOrName);
  if (!Number.isNaN(numeric) && Object.prototype.hasOwnProperty.call(ROUND_CONTRACT_ERRORS, numeric)) {
    return ROUND_CONTRACT_ERRORS[numeric];
  }
  return ROUND_CONTRACT_ERRORS_BY_NAME[codeOrName];
}

/**
 * Determine whether a contract error (by numeric code or variant name) is retryable.
 * Unknown / unmapped errors always evaluate to false (non-retryable).
 */
export function isRoundContractErrorRetryable(
  codeOrName: number | string | undefined | null,
): boolean {
  const spec = getRoundContractError(codeOrName);
  return spec !== undefined ? spec.retryable : false;
}

/**
 * Compare an external list of contract error items (e.g. from error_paths.rs, types.rs, or ERRORS.md)
 * against the SDK mapping. Returns a list of drift failure messages, or an empty array when in sync.
 */
export function diffContractErrorMapping(
  contractErrors: readonly ContractErrorEntry[],
): string[] {
  const sdkByCode = new Map(Object.values(ROUND_CONTRACT_ERRORS).map((e) => [e.code, e]));
  const inputByCode = new Map(contractErrors.map((e) => [e.code, e]));

  const failures: string[] = [];

  for (const item of contractErrors) {
    const mapped = sdkByCode.get(item.code);
    if (!mapped) {
      failures.push(
        `Contract error ${item.name} (#${item.code}) is not mapped in SDK ROUND_CONTRACT_ERRORS`,
      );
    } else if (mapped.name !== item.name) {
      failures.push(
        `Contract error code #${item.code} maps to '${mapped.name}' in SDK but '${item.name}' in contract source`,
      );
    }
  }

  for (const mapped of Object.values(ROUND_CONTRACT_ERRORS)) {
    if (!inputByCode.has(mapped.code)) {
      failures.push(
        `SDK maps error ${mapped.name} (#${mapped.code}) which is missing from contract errors list`,
      );
    }
  }

  return failures;
}

export class SubRosaClientConfigError extends Error {
  readonly name = "SubRosaClientConfigError";

  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
  }
}

export type NetworkMismatchReason =
  | "passphrase"
  | "contract_not_found"
  | "contract_mismatch"
  | "account_mismatch"
  | "session_mismatch";

export interface NetworkMismatchErrorParams {
  contractId: string;
  configuredPassphrase: string;
  rpcPassphrase?: string;
  rpcUrl?: string;
  reason: NetworkMismatchReason;
  sessionContractId?: string;
  sessionPassphrase?: string;
  sessionAccount?: string;
  expectedAccount?: string;
}

/** Raised before contract simulation/signing when network configuration or session binding conflicts. */
export class SubRosaNetworkMismatchError extends Error {
  readonly name: string = "SubRosaNetworkMismatchError";
  readonly contractId: string;
  readonly configuredPassphrase: string;
  readonly rpcPassphrase?: string;
  readonly rpcUrl?: string;
  readonly reason: NetworkMismatchErrorParams["reason"];
  readonly sessionContractId?: string;
  readonly sessionPassphrase?: string;
  readonly sessionAccount?: string;
  readonly expectedAccount?: string;

  constructor(params: NetworkMismatchErrorParams) {
    let rawMessage: string;
    if (params.reason === "passphrase") {
      rawMessage = `networkPassphrase ${JSON.stringify(params.configuredPassphrase)} does not match RPC network ${JSON.stringify(params.rpcPassphrase)} at ${params.rpcUrl}; use the passphrase and contract ID from the same deployment`;
    } else if (params.reason === "contract_not_found") {
      rawMessage = `contract ${params.contractId} was not found on RPC network ${JSON.stringify(params.rpcPassphrase)} at ${params.rpcUrl}; check that contractId and networkPassphrase refer to the same deployment`;
    } else if (params.reason === "contract_mismatch") {
      rawMessage = `passkey session contract ${params.sessionContractId ?? params.contractId} does not match client contract ${params.contractId}; refuse commit across different contracts`;
    } else if (params.reason === "account_mismatch") {
      rawMessage = `passkey session account ${params.sessionAccount} does not match client account ${params.expectedAccount}; refuse commit for different account`;
    } else {
      rawMessage = `passkey session network ${JSON.stringify(params.sessionPassphrase ?? params.configuredPassphrase)} does not match client network ${JSON.stringify(params.configuredPassphrase)}; refuse commit across different networks`;
    }

    // Keep secret seeds strictly out of error text
    const message = rawMessage.replace(/\bS[A-Z2-7]{55}\b/g, "[REDACTED]");
    super(message);
    this.contractId = params.contractId;
    this.configuredPassphrase = params.configuredPassphrase;
    this.rpcPassphrase = params.rpcPassphrase;
    this.rpcUrl = params.rpcUrl;
    this.reason = params.reason;
    this.sessionContractId = params.sessionContractId;
    this.sessionPassphrase = params.sessionPassphrase;
    this.sessionAccount = params.sessionAccount;
    this.expectedAccount = params.expectedAccount;
  }
}

/** Specific typed error for passkey session binding mismatches. Inherits from SubRosaNetworkMismatchError. */
export class SubRosaSessionMismatchError extends SubRosaNetworkMismatchError {
  override readonly name = "SubRosaSessionMismatchError";
}

export class SubRosaSubmitError extends Error {
  readonly name = "SubRosaSubmitError";
  readonly retryable?: boolean;

  constructor(message: string, options?: ErrorOptions & { retryable?: boolean }) {
    super(message, options);
    if (options?.retryable !== undefined) {
      this.retryable = options.retryable;
    }
  }
}

export class SubRosaTransactionError extends Error {
  readonly name = "SubRosaTransactionError";
  readonly hash: string;
  readonly status: string;
  readonly retryable?: boolean;

  constructor(hash: string, status: string, options?: ErrorOptions & { retryable?: boolean }) {
    super(`transaction ${hash} ended with status ${status}`, options);
    this.hash = hash;
    this.status = status;
    if (options?.retryable !== undefined) {
      this.retryable = options.retryable;
    }
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


export class SubRosaAssetValidationError extends Error {
  readonly name = "SubRosaAssetValidationError";

  constructor(readonly field: string, message: string) {
    super(`${field}: ${message}`);
  }
}

export type PreflightFailureKind =
  | "rpc_error"
  | "simulation_error"
  | "expired_state"
  | "contract_error"
  | "malformed_response";

export interface SubRosaPreflightErrorParams {
  kind: PreflightFailureKind;
  operation: string;
  message: string;
  simulationError?: string;
  contractErrorCode?: number;
  contractErrorMessage?: string;
  restoreMinResourceFee?: bigint;
  retryable?: boolean;
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
  readonly retryable: boolean;

  constructor(params: SubRosaPreflightErrorParams) {
    super(params.message, params.cause ? { cause: params.cause } : undefined);
    this.kind = params.kind;
    this.operation = params.operation;
    this.simulationError = params.simulationError;
    this.contractErrorCode = params.contractErrorCode;
    this.contractErrorMessage = params.contractErrorMessage;
    this.restoreMinResourceFee = params.restoreMinResourceFee;
    this.retryable =
      params.retryable ??
      (params.kind === "contract_error"
        ? isRoundContractErrorRetryable(params.contractErrorCode ?? params.contractErrorMessage)
        : false);
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
