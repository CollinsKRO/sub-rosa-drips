// Copyright (c) 2026 Sub Rosa contributors
// x402 paid-fetch client.
//
// Wraps a single HTTP call with the x402 handshake: try the request, and if the
// server answers 402, sign the Soroban auth entry authorizing the USDC transfer
// and retry with the `X-PAYMENT` header. Returns both the resource body and the
// on-chain settlement receipt. This is what an autonomous bidder agent uses to
// pay the appraisal API per call.

import { x402Client, x402HTTPClient } from "@x402/core/client";
import type { Network, PaymentRequired, SettleResponse } from "@x402/core/types";
import { createEd25519Signer } from "@x402/stellar";
import { ExactStellarScheme as ClientStellarScheme } from "@x402/stellar/exact/client";

export interface PaidClientConfig {
  /** Payer secret key (S...). Needs a USDC trustline + balance. */
  secret: string;
  /** CAIP-2 network id (default stellar:testnet). */
  network?: Network;
  /** Optional custom Soroban RPC URL. */
  rpcUrl?: string;
}

export interface PaidResult<T = unknown> {
  status: number;
  body: T;
  /** Present when a payment was made and settled on-chain. */
  settlement?: SettleResponse;
}

export const MAX_PAYMENT_ERROR_DIAGNOSTIC_LENGTH = 512;
const SENSITIVE_FIELD = /("?(?:secret|token|password|authorization|privateKey|private_key|apiKey|api_key)"?\s*:\s*)"?[^,}\s]+/gi;

/** Bound provider diagnostics and redact common credential fields before display/logging. */
export function sanitizePaymentErrorDiagnostic(body: string): string {
  return body.slice(0, MAX_PAYMENT_ERROR_DIAGNOSTIC_LENGTH).replace(SENSITIVE_FIELD, '$1[REDACTED]').slice(0, MAX_PAYMENT_ERROR_DIAGNOSTIC_LENGTH);
}

export class AppraisalResponseParseError extends Error {
  readonly name = "AppraisalResponseParseError";
  readonly status: number;

  constructor(status: number, options?: ErrorOptions) {
    super(`appraisal api returned ${status} with invalid JSON body`, options);
    this.status = status;
  }
}

export class X402PaymentError extends Error {
  readonly name = "X402PaymentError";
  readonly status?: number;

  constructor(message: string, status?: number) {
    super(message);
    this.status = status;
  }
}

/** Typed error raised when a 402 challenge does not match the client's own request. */
export class QuoteMismatchError extends Error {
  readonly name = "QuoteMismatchError";
  readonly reason: QuoteMismatchReason;
  readonly status?: number;

  constructor(reason: QuoteMismatchReason, status?: number) {
    super(quoteMismatchMessage(reason));
    this.reason = reason;
    this.status = status;
  }
}

export type QuoteMismatchReason =
  | "empty-challenge"
  | "expired"
  | "asset-mismatch"
  | "amount-mismatch"
  | "destination-mismatch";

function quoteMismatchMessage(reason: QuoteMismatchReason): string {
  switch (reason) {
    case "empty-challenge":
      return "x402 challenge contained no payable quote";
    case "expired":
      return "x402 challenge expired before payment";
    case "asset-mismatch":
      return "x402 challenge asset does not match the requested asset";
    case "amount-mismatch":
      return "x402 challenge amount does not match the requested amount";
    case "destination-mismatch":
      return "x402 challenge destination does not match the requested destination";
  }
}

/** The quote the client expects to pay for a given request. */
export interface ExpectedQuote {
  asset: string;
  amount: bigint;
  destination: string;
  /** Unix seconds; the challenge must not be expired at payment time. */
  expiresAt: number;
}

export interface PaidFetchOptions {
  /** The quote this call is willing to pay. */
  expectedQuote?: ExpectedQuote;
  /** Override the clock used for expiry checks (tests). */
  nowSeconds?: () => number;
}

async function parseJsonResponse<T>(res: Response): Promise<T> {
  const text = await res.text();
  if (!text.trim()) {
    throw new AppraisalResponseParseError(res.status);
  }

  try {
    return JSON.parse(text) as T;
  } catch (cause) {
    throw new AppraisalResponseParseError(res.status, { cause });
  }
}

/** Normalize a quote amount (any number or string form) to bigint stroips. */
function normalizeQuoteAmount(value: unknown): bigint | undefined {
  if (typeof value === "bigint") return value;
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) return undefined;
    return BigInt(value);
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!/^\d+$/.test(trimmed)) return undefined;
    try {
      return BigInt(trimmed);
    } catch {
      return undefined;
    }
  }
  return undefined;
}

/** Normalize an expiry timestamp to Unix seconds. */
function normalizeExpiry(value: unknown): number | undefined {
  if (typeof value === "number") {
    if (!Number.isFinite(value)) return undefined;
    // Millisecond epochs are common in JSON payloads; convert to seconds.
    return value > 1e12 ? Math.floor(value / 1000) : Math.floor(value);
  }
  if (typeof value === "string") {
    const trimmed = value.trim();
    if (!/^\d+$/.test(trimmed)) return undefined;
    const num = Number(trimmed);
    if (!Number.isFinite(num)) return undefined;
    return num > 1e12 ? Math.floor(num / 1000) : Math.floor(num);
  }
  return undefined;
}

/** Find the first accepts entry that carries a payable quote. */
function firstQuote(paymentRequired: PaymentRequired): Record<string, unknown> | undefined {
  const accepts = (paymentRequired as { accepts?: unknown }).accepts;
  if (!Array.isArray(accepts)) return undefined;
  for (const entry of accepts) {
    if (entry && typeof entry === "object") {
      return entry as Record<string, unknown>;
    }
  }
  return undefined;
}

function quoteAsset(entry: Record<string, unknown>): string | undefined {
  const asset = entry.asset;
  if (typeof asset === "string" && asset.trim() !== "") return asset;
  const currency = entry.currency;
  if (typeof currency === "string" && currency.trim() !== "") return currency;
  return undefined;
}

function quoteDestination(entry: Record<string, unknown>): string | undefined {
  for (const key of ["destination", "payTo", "pay_to", "recipient", "address"] as const) {
    const value = entry[key];
    if (typeof value === "string" && value.trim() !== "") return value;
  }
  return undefined;
}

function quoteAmount(entry: Record<string, unknown>): bigint | undefined {
  for (const key of ["amount", "maxAmountRequired", "max_amount_required", "price", "value"] as const) {
    const normalized = normalizeQuoteAmount(entry[key]);
    if (normalized !== undefined) return normalized;
  }
  return undefined;
}

function quoteExpiry(entry: Record<string, unknown>): number | undefined {
  for (const key of ["expiresAt", "expires_at", "expiration", "expires"] as const) {
    const normalized = normalizeExpiry(entry[key]);
    if (normalized !== undefined) return normalized;
  }
  return undefined;
}

/** Verify the 402 challenge against the quote the client requested. */
export function assertChallengeMatchesQuote(
  paymentRequired: PaymentRequired,
  expected: ExpectedQuote,
  nowSeconds: () => number = () => Math.floor(Date.now() / 1000),
): void {
  const entry = firstQuote(paymentRequired);
  if (!entry) {
    throw new QuoteMismatchError("empty-challenge");
  }

  const now = nowSeconds();
  if (!Number.isFinite(now)) {
    throw new QuoteMismatchError("expired");
  }
  if (expected.expiresAt <= now) {
    throw new QuoteMismatchError("expired");
  }

  const challengeExpiry = quoteExpiry(entry);
  if (challengeExpiry === undefined || challengeExpiry <= now) {
    throw new QuoteMismatchError("expired");
  }

  const challengeAsset = quoteAsset(entry);
  if (challengeAsset === undefined || challengeAsset !== expected.asset) {
    throw new QuoteMismatchError("asset-mismatch");
  }

  const challengeAmount = quoteAmount(entry);
  if (challengeAmount === undefined || challengeAmount !== expected.amount) {
    throw new QuoteMismatchError("amount-mismatch");
  }

  const challengeDestination = quoteDestination(entry);
  if (challengeDestination === undefined || challengeDestination !== expected.destination) {
    throw new QuoteMismatchError("destination-mismatch");
  }
}

/** Build a paid-fetch function bound to a payer wallet. */
export function createPaidFetch(config: PaidClientConfig) {
  const network = config.network ?? "stellar:testnet";
  const signer = createEd25519Signer(config.secret, network);
  const rpcConfig = config.rpcUrl ? { url: config.rpcUrl } : undefined;
  const core = new x402Client().register(
    "stellar:*",
    new ClientStellarScheme(signer, rpcConfig),
  );
  const http = new x402HTTPClient(core);

  return async function paidFetch<T = unknown>(
    url: string,
    init: RequestInit = {},
    options: PaidFetchOptions = {},
  ): Promise<PaidResult<T>> {
    const first = await fetch(url, init);
    if (first.status !== 402) {
      return { status: first.status, body: await parseJsonResponse<T>(first) };
    }

    // 402 → build the signed payment and retry.
    let bodyForParse: unknown;
    try {
      bodyForParse = await parseJsonResponse<unknown>(first.clone());
    } catch (error) {
      if (error instanceof AppraisalResponseParseError) {
        bodyForParse = undefined;
      } else {
        throw error;
      }
    }

    let paymentRequired: PaymentRequired;
    try {
      paymentRequired = http.getPaymentRequiredResponse(
        (name) => first.headers.get(name),
        bodyForParse,
      );
    } catch {
      throw new QuoteMismatchError("empty-challenge", first.status);
    }

    // Bind the payment to the quote this call requested. A changed asset,
    // amount, destination, or expiry must not be paid.
    if (!options.expectedQuote) {
      throw new QuoteMismatchError("empty-challenge", first.status);
    }
    assertChallengeMatchesQuote(paymentRequired, options.expectedQuote, options.nowSeconds);

    const payload = await http.createPaymentPayload(paymentRequired);
    const payHeaders = http.encodePaymentSignatureHeader(payload);

    const paid = await fetch(url, {
      ...init,
      headers: { ...(init.headers ?? {}), ...payHeaders },
    });
    const body = await parseJsonResponse<T>(paid);

    if (paid.status !== 200) {
      throw new X402PaymentError(`paid request failed (${paid.status})`, paid.status);
    }
    const settlement = http.getPaymentSettleResponse((name) => paid.headers.get(name));
    return { status: paid.status, body, settlement };
  };
}
