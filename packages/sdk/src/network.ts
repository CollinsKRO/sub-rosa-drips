import { Contract, rpc } from "@stellar/stellar-sdk";
import {
  SubRosaNetworkMismatchError,
  SubRosaSessionMismatchError,
} from "./errors.js";
import { normalizeSorobanContractId } from "./ids.js";

export type NetworkValidationServer = Pick<
  rpc.Server,
  "getNetwork" | "getLedgerEntries"
>;

export interface ContractNetworkValidationConfig {
  networkPassphrase: string;
  contractId: string;
  rpcUrl: string;
}

export interface PasskeySessionBinding {
  /** Contract ID the passkey session was started for. */
  contractId: string;
  /** Network passphrase the passkey session was created on. */
  networkPassphrase: string;
  /** Account/public key bound to this session. */
  account?: string;
}

export interface SessionBindingTarget {
  /** Target contract ID (e.g., from the SDK client). */
  contractId: string;
  /** Target network passphrase (e.g., from the SDK client). */
  networkPassphrase: string;
  /** Target account (e.g., from the SDK client). */
  account?: string;
}

/**
 * Validate that a passkey session matches the contract ID, network passphrase,
 * and account of the executing SDK client.
 *
 * Refuses execution and throws SubRosaSessionMismatchError if any differ.
 */
export function validatePasskeySession(
  session: PasskeySessionBinding,
  target: SessionBindingTarget,
): void {
  if (session.networkPassphrase !== target.networkPassphrase) {
    throw new SubRosaSessionMismatchError({
      contractId: target.contractId,
      configuredPassphrase: target.networkPassphrase,
      sessionPassphrase: session.networkPassphrase,
      reason: "session_mismatch",
    });
  }

  let sessionContract = session.contractId.trim();
  let targetContract = target.contractId.trim();
  try {
    sessionContract = normalizeSorobanContractId(session.contractId);
  } catch {
    // Keep trimmed string
  }
  try {
    targetContract = normalizeSorobanContractId(target.contractId);
  } catch {
    // Keep trimmed string
  }

  if (sessionContract !== targetContract) {
    throw new SubRosaSessionMismatchError({
      contractId: targetContract,
      configuredPassphrase: target.networkPassphrase,
      sessionContractId: sessionContract,
      reason: "contract_mismatch",
    });
  }

  if (session.account && target.account && session.account !== target.account) {
    throw new SubRosaSessionMismatchError({
      contractId: targetContract,
      configuredPassphrase: target.networkPassphrase,
      sessionAccount: session.account,
      expectedAccount: target.account,
      reason: "account_mismatch",
    });
  }
}

/**
 * Confirm that the RPC is on the configured network and that the contract
 * exists there. Contract StrKeys do not encode a network, so both checks are
 * required to catch a contract ID copied from another deployment.
 */
export async function validateContractNetwork(
  server: NetworkValidationServer,
  config: ContractNetworkValidationConfig,
): Promise<void> {
  const network = await server.getNetwork();
  if (network.passphrase !== config.networkPassphrase) {
    throw new SubRosaNetworkMismatchError({
      contractId: config.contractId,
      configuredPassphrase: config.networkPassphrase,
      rpcPassphrase: network.passphrase,
      rpcUrl: config.rpcUrl,
      reason: "passphrase",
    });
  }

  const contractKey = new Contract(config.contractId).getFootprint();
  const response = await server.getLedgerEntries(contractKey);
  if (response.entries.length === 0) {
    throw new SubRosaNetworkMismatchError({
      contractId: config.contractId,
      configuredPassphrase: config.networkPassphrase,
      rpcPassphrase: network.passphrase,
      rpcUrl: config.rpcUrl,
      reason: "contract_not_found",
    });
  }
}

