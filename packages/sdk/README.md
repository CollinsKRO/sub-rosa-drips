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
