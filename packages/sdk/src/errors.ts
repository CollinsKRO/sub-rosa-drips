// SPDX-License-Identifier: MIT
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
  | "malformed_response";

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
