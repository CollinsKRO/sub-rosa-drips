# `@sub-rosa/sdk`

TypeScript client for reading and submitting Sub Rosa Round contract calls.

## Bidder enumeration

`client.bidders(roundId)` follows the contract's opaque cursors until `has_more`
is false. Each bidder is yielded once in first-commit order. A repeated bidder
or a page that cannot make consistent progress throws `SubRosaPaginationError`;
consumers must let that error abort the operation rather than use a partial set.
Receipt export uses this iterator too.

For manual paging, call `getBiddersPage(roundId, undefined, limit)` to start,
then pass `page.next_cursor` unchanged while `page.has_more` is true. The first
page fixes a snapshot count, excluding bidders who commit later; restart to
include those bidders. Tokens from another round or contract are rejected.
The [cursor format](../../contracts/round/ERRORS.md#bidder-cursor-encoding-v1)
is versioned and replaces the old numeric-offset ABI, so this SDK requires a
contract deployed with the matching generated bindings.

## Network configuration

Configure the RPC URL, network passphrase, and contract ID from the same deployment:

```ts
import { SubRosaClient } from "@sub-rosa/sdk";

const client = new SubRosaClient({
  rpcUrl: "https://soroban-testnet.stellar.org",
  networkPassphrase: "Test SDF Network ; September 2015",
  contractId: process.env.ROUND_CONTRACT_ID!,
  publicKey: process.env.STELLAR_PUBLIC_KEY,
});
```

On the first contract call, the client asks the RPC for its actual network
passphrase and confirms that `contractId` exists on that network. The result is
cached for later calls. A mismatch throws `SubRosaNetworkMismatchError` before
simulation, signing, or submission, with the conflicting values and a suggested
fix. Contract IDs do not encode a Stellar network, so copying a `C...` address
between Testnet and Mainnet requires updating all three configuration values.

## Escrow conservation preflight

The contract keeps one identity per round — the escrow it holds equals the
payout plus refunds plus whatever is still locked — and refuses to `settle` or
`void` a round that cannot prove it, failing with `EscrowNotConserved`. The SDK
re-derives the same accounting off-chain so a keeper can halt before paying a
fee.

`proveEscrowConservation` walks the bidder index in pages, reads every bid
state, and cross-checks the walk against the bidder list on the round record. It
never throws; a drifted, duplicated, or unreadable index comes back as an issue
on the report:

```ts
const report = await client.proveEscrowConservation(roundId, "settle");
if (!report.conserved) {
  for (const issue of report.issues) console.warn(issue.code, issue.message);
}
```

`preflightSettleConservation` and `preflightVoidConservation` are the stricter
wrappers: they throw `SubRosaEscrowConservationError` (a `SubRosaPreflightError`
with `kind: "escrow_not_conserved"`) carrying the `roundId`, the `phase`, and
the full report, so a keeper can branch on `error.kind` instead of parsing text.

```ts
try {
  await client.preflightSettleConservation(roundId);
  await client.settle(roundId);
} catch (error) {
  if (error instanceof SubRosaEscrowConservationError) {
    console.error(error.roundId, error.phase, error.report.issues);
  }
}
```

Issue codes cover page drift (`page_total_drift`, `page_count_mismatch`,
`cursor_stalled`), index integrity (`duplicate_bidder`, `index_mismatch`,
`bid_state_missing`, `bidder_already_settled`, `winner_not_indexed`), and the
accounting itself (`escrow_stranded`, `round_wrong_status`, `no_winner`).
