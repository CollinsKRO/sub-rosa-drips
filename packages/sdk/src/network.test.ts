import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { StrKey } from "@stellar/stellar-sdk";
import { SubRosaNetworkMismatchError } from "./errors.js";
import {
  validateContractNetwork,
  networkDisplayName,
  networkPassphrasesMatch,
  describeNetworkPassphraseMismatch,
  SubRosaNetworkPassphraseMismatchError,
  type NetworkValidationServer,
} from "./network.js";

const TESTNET = "Test SDF Network ; September 2015";
const PUBLIC = "Public Global Stellar Network ; September 2015";
const CONTRACT_ID = StrKey.encodeContract(Buffer.alloc(32));
const RPC_URL = "https://rpc.example.test";

function server(
  passphrase: string,
  contractExists = true,
): NetworkValidationServer {
  return {
    getNetwork: async () => ({
      passphrase,
      protocolVersion: "23",
    }),
    getLedgerEntries: async () => ({
      entries: contractExists ? [{} as never] : [],
      latestLedger: 123,
    }),
  } as NetworkValidationServer;
}

const config = (networkPassphrase: string) => ({
  networkPassphrase,
  contractId: CONTRACT_ID,
  rpcUrl: RPC_URL,
});

describe("validateContractNetwork", () => {
  it("accepts matched testnet and public-network configurations", async () => {
    await assert.doesNotReject(
      validateContractNetwork(server(TESTNET), config(TESTNET)),
    );
    await assert.doesNotReject(
      validateContractNetwork(server(PUBLIC), config(PUBLIC)),
    );
  });

  it("rejects a testnet passphrase against a public-network RPC", async () => {
    await assert.rejects(
      validateContractNetwork(server(PUBLIC), config(TESTNET)),
      (error: unknown) => {
        assert.ok(error instanceof SubRosaNetworkMismatchError);
        assert.equal(error.reason, "passphrase");
        assert.equal(error.configuredPassphrase, TESTNET);
        assert.equal(error.rpcPassphrase, PUBLIC);
        assert.match(error.message, /same deployment/);
        return true;
      },
    );
  });

  it("rejects a public passphrase against a testnet RPC", async () => {
    await assert.rejects(
      validateContractNetwork(server(TESTNET), config(PUBLIC)),
      SubRosaNetworkMismatchError,
    );
  });

  it("rejects a contract that is absent from the configured network", async () => {
    await assert.rejects(
      validateContractNetwork(server(TESTNET, false), config(TESTNET)),
      (error: unknown) => {
        assert.ok(error instanceof SubRosaNetworkMismatchError);
        assert.equal(error.reason, "contract_not_found");
        assert.equal(error.contractId, CONTRACT_ID);
        assert.match(error.message, /contractId and networkPassphrase/);
        return true;
      },
    );
  });

  it("labels known and custom network passphrases", () => {
    assert.equal(networkDisplayName(TESTNET), "Testnet");
    assert.equal(networkDisplayName(PUBLIC), "Public");
    assert.equal(networkDisplayName("Futurenet"), "Futurenet");
    assert.equal(networkDisplayName("Custom Network"), "Custom Network");
    assert.equal(networkDisplayName("   "), "an unknown network");
  });

  it("matches passphrases after trimming", () => {
    assert.equal(networkPassphrasesMatch(TESTNET, ` ${TESTNET} `), true);
    assert.equal(networkPassphrasesMatch(TESTNET, PUBLIC), false);
  });

  it("names both networks on a passphrase mismatch without exposing a secret", () => {
    const error = new SubRosaNetworkPassphraseMismatchError(PUBLIC, TESTNET);
    assert.equal(error.chainNetwork, "Public");
    assert.equal(error.sdkNetwork, "Testnet");
    assert.equal(error.message, describeNetworkPassphraseMismatch(PUBLIC, TESTNET));
    assert.match(error.message, /Public/);
    assert.match(error.message, /Testnet/);
    assert.doesNotMatch(error.message, /\bS[A-Z2-7]{55}\b/);
    assert.doesNotMatch(error.message, /AAAA/);
  });

  it("does not look up the contract after a passphrase mismatch", async () => {
    let ledgerLookups = 0;
    const mock = server(PUBLIC);
    mock.getLedgerEntries = async () => {
      ledgerLookups += 1;
      return { entries: [], latestLedger: 123 };
    };

    await assert.rejects(
      validateContractNetwork(mock, config(TESTNET)),
      SubRosaNetworkMismatchError,
    );
    assert.equal(ledgerLookups, 0);
  });
});
