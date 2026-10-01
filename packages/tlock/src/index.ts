// Copyright (c) 2026 Sub Rosa contributors
export {
  commitment,
  encodeBidPreimage,
  decodeBidPreimage,
  i128ToBeBytes,
  beBytesToI128,
  toHex,
  fromHex,
  isValidHex,
  VALUE_BYTES,
  NONCE_BYTES,
  PREIMAGE_BYTES,
} from "./commitment.js";

export {
  generateAuditorKeypair,
  auditorPublicKey,
  sealIdentity,
  openIdentity,
  sealIdentityForBidder,
  openIdentityForBidder,
  decodeIdentityBlob,
  isIdentityBound,
  IdentityBindingError,
  IDENTITY_BLOB_VERSION,
  type AuditorKeypair,
  type SealIdentityForBidderParams,
  type OpenIdentityForBidderParams,
  type OpenedIdentity,
} from "./auditor.js";

export {
  quicknet,
  chainInfo,
  currentRound,
  roundInSeconds,
  fetchRoundBeacon,
  fetchRoundSignature,
  QUICKNET_HASH,
  type DrandClient,
} from "./quicknet.js";

export { drandSignatureToSoroban, encodeG1Soroban } from "./bls.js";

export {
  assertChainInfo,
  assertBeacon,
  type RawChainInfo,
  type RawBeacon,
} from "./validate.js";

export {
  sealBid,
  openBid,
  generateNonce,
  type SealBidParams,
  type SealedBid,
  type OpenedBid,
} from "./seal.js";

export {
  classifyDrandRound,
  DEFAULT_STALE_THRESHOLD_MS,
  type DrandRoundInfo,
  type FreshnessStatus,
  type FreshnessResult,
} from "./freshness.js";

export {
  encodePayloadEnvelope,
  decodePayloadEnvelope,
  payloadCommitment,
  sealPayload,
  openPayload,
  PAYLOAD_ENVELOPE_VERSION,
  PAYLOAD_HEADER_BYTES,
  MAX_APPLICATION_PAYLOAD_BYTES,
  type PayloadEnvelope,
  type SealPayloadParams,
  type SealedPayload,
} from "./payload.js";
