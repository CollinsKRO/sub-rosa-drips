import { test } from "node:test";
import assert from "node:assert/strict";

import {
  beBytesToI128,
  commitment,
  commitmentMatches,
  decodeBidPreimage,
  encodeBidPreimage,
  i128ToBeBytes,
  toHex,
  COMMITMENT_BYTES,
  NONCE_BYTES,
} from "./commitment.js";

// Frozen vector shared with the Round contract's Rust test
// (`commitment_matches_offchain_vector`). This is the single source of truth
// that off-chain H == on-chain sha256(value ‖ nonce) over identical bytes.
const FROZEN_VALUE = 700n;
const FROZEN_NONCE = new Uint8Array(32).fill(0x11);
const FROZEN_PREIMAGE =
  "000000000000000000000000000002bc" + "11".repeat(32);
const FROZEN_H =
  "3d4c2d3604b23250687f0344a9474e3c748742a4fba4616d308d529121a8dec4";

test("frozen commitment vector matches the contract (cross-language parity)", () => {
  assert.equal(toHex(encodeBidPreimage(FROZEN_VALUE, FROZEN_NONCE)), FROZEN_PREIMAGE);
  assert.equal(toHex(commitment(FROZEN_VALUE, FROZEN_NONCE)), FROZEN_H);
});

test("preimage encode/decode roundtrip", () => {
  const pre = encodeBidPreimage(FROZEN_VALUE, FROZEN_NONCE);
  const { value, nonce } = decodeBidPreimage(pre);
  assert.equal(value, FROZEN_VALUE);
  assert.deepEqual([...nonce], [...FROZEN_NONCE]);
});

test("i128 big-endian encode/decode incl. large values", () => {
  for (const v of [0n, 1n, 700n, 1_000_000n, (1n << 126n)]) {
    assert.equal(beBytesToI128(i128ToBeBytes(v)), v);
  }
});

test("wrong nonce or value yields a different commitment", () => {
  const h = toHex(commitment(FROZEN_VALUE, FROZEN_NONCE));
  const hWrongNonce = toHex(commitment(FROZEN_VALUE, new Uint8Array(32).fill(0x99)));
  const hWrongValue = toHex(commitment(701n, FROZEN_NONCE));
  assert.notEqual(h, hWrongNonce);
  assert.notEqual(h, hWrongValue);
});

test("rejects out-of-range and malformed inputs", () => {
  assert.throws(() => i128ToBeBytes(1n << 127n)); // > i128 max
  assert.throws(() => encodeBidPreimage(1n, new Uint8Array(31)));
  assert.throws(() => decodeBidPreimage(new Uint8Array(47)));
});

// ── commitmentMatches — the one acceptance rule for "is this H this bid" ───

test("commitmentMatches accepts the commitment the sealer derives for that value and nonce", () => {
  const h = commitment(FROZEN_VALUE, FROZEN_NONCE);
  assert.equal(h.length, COMMITMENT_BYTES);
  assert.equal(commitmentMatches(FROZEN_VALUE, FROZEN_NONCE, h), true);
  // The comparison is over exact bytes: H is an opaque digest, so reordering
  // it yields a different value and must not be mistaken for a match.
  assert.equal(commitmentMatches(FROZEN_VALUE, FROZEN_NONCE, h.slice().reverse()), false);
});

test("commitmentMatches rejects a swapped value, nonce, or commitment", () => {
  const h = commitment(FROZEN_VALUE, FROZEN_NONCE);
  // A different value with the same nonce.
  assert.equal(commitmentMatches(FROZEN_VALUE + 1n, FROZEN_NONCE, h), false);
  // The same value with a different nonce.
  assert.equal(commitmentMatches(FROZEN_VALUE, new Uint8Array(32).fill(0x99), h), false);
  // Another bid's commitment entirely.
  assert.equal(commitmentMatches(FROZEN_VALUE, FROZEN_NONCE, commitment(701n, FROZEN_NONCE)), false);
  // A single flipped bit.
  const flipped = h.slice();
  flipped[0] ^= 0x01;
  assert.equal(commitmentMatches(FROZEN_VALUE, FROZEN_NONCE, flipped), false);
});

test("commitmentMatches treats wrong-width inputs as a mismatch rather than throwing", () => {
  const h = commitment(FROZEN_VALUE, FROZEN_NONCE);
  assert.equal(commitmentMatches(FROZEN_VALUE, FROZEN_NONCE, h.slice(0, 31)), false);
  assert.equal(commitmentMatches(FROZEN_VALUE, FROZEN_NONCE, new Uint8Array(0)), false);
  assert.equal(
    commitmentMatches(FROZEN_VALUE, new Uint8Array(31), h),
    false,
    `a nonce that is not ${NONCE_BYTES} bytes cannot produce this H`,
  );
});

test("commitmentMatches holds across the i128 boundary values", () => {
  for (const value of [0n, 1n, -1n, FROZEN_VALUE, (1n << 126n), -(1n << 126n)]) {
    assert.equal(commitmentMatches(value, FROZEN_NONCE, commitment(value, FROZEN_NONCE)), true);
  }
});
