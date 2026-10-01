import { Contract, rpc } from "@stellar/stellar-sdk";
import { SubRosaNetworkMismatchError } from "./errors.js";

/**
 * Human-readable label for a Stellar network passphrase. Unknown/custom
 * networks fall back to the passphrase itself so the caller still sees which
 * network it is (never a secret).
 */
export function networkDisplayName(passphrase: string): string {
  const trimmed = passphrase.trim();
  if (trimmed === "") return "an unknown network";
  if (trimmed.includes("Public")) return "Public";
  if (trimmed.includes("Future")) return "Futurenet";
  if (trimmed.includes("Test SDF") || /^Test\b/.test(trimmed)) return "Testnet";
  if (trimmed.includes("Standalone") || trimmed.includes("Local")) return "Standalone";
  return trimmed;
}

/** True when both passphrases refer to the same network (after trimming). */
export function networkPassphrasesMatch(
  chainPassphrase: string,
  sdkPassphrase: string,
): boolean {
  return chainPassphrase.trim() === sdkPassphrase.trim();
}

/**
 * Message that names both networks so the operator knows which wallet switch
 * to make. Contains only network labels — no secret keys or signed envelopes.
 */
export function describeNetworkPassphraseMismatch(
  chainPassphrase: string,
  sdkPassphrase: string,
): string {
  return (
    `network passphrase mismatch: the connected chain is on ` +
    `"${networkDisplayName(chainPassphrase)}" but the SDK client is configured for ` +
    `"${networkDisplayName(sdkPassphrase)}"`
  );
}

/**
 * Raised before signing/submitting when the connected chain's passphrase
 * differs from the one the SDK client was configured with. Carries both
 * network labels so callers can surface them without exposing a secret or a
 * signed XDR.
 */
export class SubRosaNetworkPassphraseMismatchError extends Error {
  readonly name = "SubRosaNetworkPassphraseMismatchError";
  readonly chainPassphrase: string;
  readonly sdkPassphrase: string;
  readonly chainNetwork: string;
  readonly sdkNetwork: string;

  constructor(chainPassphrase: string, sdkPassphrase: string) {
    super(describeNetworkPassphraseMismatch(chainPassphrase, sdkPassphrase));
    this.chainPassphrase = chainPassphrase;
    this.sdkPassphrase = sdkPassphrase;
    this.chainNetwork = networkDisplayName(chainPassphrase);
    this.sdkNetwork = networkDisplayName(sdkPassphrase);
  }
}

export type NetworkValidationServer = Pick<
  rpc.Server,
  "getNetwork" | "getLedgerEntries"
>;

export interface ContractNetworkValidationConfig {
  networkPassphrase: string;
  contractId: string;
  rpcUrl: string;
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
