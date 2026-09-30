#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Bytes, BytesN, ConversionError, Env, InvokeError, Vec,
};
use soroban_sdk::testutils::storage::Temporary as TemporaryStorageTest;

use crate::drand;
use crate::storage::{seal_ttl_for_reveal_deadline, TEMP_THRESHOLD};
use crate::types::{ClearingRule, DataKey, Error, GlobalConfig, Status};
use crate::{SubRosaRound, SubRosaRoundClient};

// ── Dummy fixture (no BLS) — only for tests that never call open_reveal ──────
const GENESIS: u64 = 0;
const PERIOD: u64 = 1;

struct Fixture {
    env: Env,
    client: SubRosaRoundClient<'static>,
    usdc_admin: token::StellarAssetClient<'static>,
    usdc_token: token::Client<'static>,
}

fn setup() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|l| l.timestamp = 1_000);

    let issuer = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(issuer);
    let usdc = sac.address();

    let drand_pubkey = BytesN::from_array(&env, &[0u8; 192]);
    let g2_neg_generator = BytesN::from_array(&env, &[0u8; 192]);
    let dst = Bytes::from_array(&env, b"BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_NUL_");

    let contract_id = env.register(
        SubRosaRound,
        (drand_pubkey, g2_neg_generator, dst, GENESIS, PERIOD, usdc.clone()),
    );
    let client = SubRosaRoundClient::new(&env, &contract_id);

    Fixture {
        env: env.clone(),
        client,
        usdc_admin: token::StellarAssetClient::new(&env, &usdc),
        usdc_token: token::Client::new(&env, &usdc),
    }
}

// ── Real Drand constants (quicknet round 29155653) ───────────────────────────
// All tests that go through open_reveal must use this fixture + VEC_SIG.
pub(super) const VEC_ROUND: u64 = 29_155_653;
const VEC_SIG_G1: &str = "0f74ee9ea1bc8ab52cc375ec82e70b6fed483a2618e90eeaef5631555733554f8bb3ec7c8563341af525d09b3702cae7181d281dbcb68e4779e93184eea8f879301f980708c26e488b5417f9c257b6b9cee7f9a2d6981fb65b7bcd6bcc15d3ac";
const VEC_PUBKEY_C1C0: &str = "03cf0f2896adee7eb8b5f01fcad3912212c437e0073e911fb90022d3e760183c8c4b450b6a0a6c3ac6a5776a2d1064510d1fec758c921cc22b0e17e63aaf4bcb5ed66304de9cf809bd274ca73bab4af5a6e9c76a4bc09e76eae8991ef5ece45a01a714f2edb74119a2f2b0d5a7c75ba902d163700a61bc224ededd8e63aef7be1aaf8e93d7a9718b047ccddb3eb5d68b0e5db2b6bfbb01c867749cadffca88b36c24f3012ba09fc4d3022c5c37dce0f977d3adb5d183c7477c442b1f04515273";
const VEC_NEGGEN_C1C0: &str = "13e02b6052719f607dacd3a088274f65596bd0d09920b61ab5da61bbdc7f5049334cf11213945d57e5ac7d055d042b7e024aa2b2f08f0a91260805272dc51051c6e47ad4fa403b02b4510b647ae3d1770bac0326a805bbefd48056c8c121bdb813fa4d4a0ad8b1ce186ed5061789213d993923066dddaf1040bc3ff59f825c78df74f2d75467e25e0f55f8a00fa030ed0d1b3cc2c7027888be51d9ef691d77bcb679afda66c73f17f9ee3837a55024f78c71363275a75d75d86bab79f74782aa";
const VEC_DST: &[u8] = b"BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_NUL_";
const VEC_GENESIS: u64 = 1_692_803_367;
const VEC_PERIOD: u64 = 3;

fn hexval(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("bad hex"),
    }
}

fn hexn<const N: usize>(env: &Env, s: &str) -> BytesN<N> {
    let raw = s.as_bytes();
    assert_eq!(raw.len(), N * 2, "hex length mismatch");
    let mut out = [0u8; N];
    let mut i = 0;
    while i < N {
        out[i] = (hexval(raw[i * 2]) << 4) | hexval(raw[i * 2 + 1]);
        i += 1;
    }
    BytesN::from_array(env, &out)
}

fn config_with(env: &Env, pubkey: &str, neg_gen: &str) -> GlobalConfig {
    GlobalConfig {
        drand_pubkey: hexn::<192>(env, pubkey),
        g2_neg_generator: hexn::<192>(env, neg_gen),
        dst: Bytes::from_slice(env, VEC_DST),
        drand_genesis: VEC_GENESIS,
        drand_period: VEC_PERIOD,
        usdc: Address::generate(env),
    }
}

/// Fixture backed by the real quicknet BLS keys.
/// commit_deadline  = time(VEC_ROUND) - 100
/// reveal_deadline  = time(VEC_ROUND) + 200
/// Ledger starts at time(VEC_ROUND) - 200 (commit window open).
fn setup_drand() -> (Fixture, u64, u64, u64) {
    let env = Env::default();
    env.mock_all_auths();

    let issuer = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(issuer);
    let usdc = sac.address();

    let contract_id = env.register(
        SubRosaRound,
        (
            hexn::<192>(&env, VEC_PUBKEY_C1C0),
            hexn::<192>(&env, VEC_NEGGEN_C1C0),
            Bytes::from_slice(&env, VEC_DST),
            VEC_GENESIS,
            VEC_PERIOD,
            usdc.clone(),
        ),
    );
    let client = SubRosaRoundClient::new(&env, &contract_id);

    let t_reveal = VEC_GENESIS + VEC_PERIOD * VEC_ROUND;
    let commit_deadline = t_reveal - 100;
    let reveal_deadline = t_reveal + 200;
    env.ledger().with_mut(|l| l.timestamp = t_reveal - 200);

    let f = Fixture {
        env: env.clone(),
        client,
        usdc_admin: token::StellarAssetClient::new(&env, &usdc),
        usdc_token: token::Client::new(&env, &usdc),
    };
    (f, t_reveal, commit_deadline, reveal_deadline)
}

/// Open a round using the real drand fixture timing.
fn drand_round(f: &Fixture, operator: &Address, commit_deadline: u64, reveal_deadline: u64, rule: ClearingRule) -> u64 {
    f.client.create_round(
        operator,
        &b32(&f.env, 0xAB),
        &VEC_ROUND,
        &rule,
        &commit_deadline,
        &reveal_deadline,
        &Bytes::from_array(&f.env, b"auditor"),
    )
}

fn real_sig(env: &Env) -> BytesN<96> {
    hexn::<96>(env, VEC_SIG_G1)
}

// ── Shared helpers ────────────────────────────────────────────────────────────

fn funded_bidder(f: &Fixture, amount: i128) -> Address {
    let bidder = Address::generate(&f.env);
    f.usdc_admin.mint(&bidder, &amount);
    bidder
}

fn b32(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn open_round(f: &Fixture, operator: &Address) -> u64 {
    f.client.create_round(
        operator,
        &b32(&f.env, 1),
        &2_000,
        &ClearingRule::HighestBid,
        &1_500,
        &2_500,
        &Bytes::from_array(&f.env, b"auditor-pubkey"),
    )
}

fn commitment(env: &Env, value: i128, nonce: &BytesN<32>) -> BytesN<32> {
    let mut pre = Bytes::new(env);
    pre.extend_from_array(&value.to_be_bytes());
    pre.extend_from_array(&nonce.to_array());
    env.crypto().sha256(&pre).to_bytes()
}

fn commit_bid(f: &Fixture, round_id: u64, bidder: &Address, value: i128, escrow: i128, nonce_byte: u8) -> BytesN<32> {
    let nonce = b32(&f.env, nonce_byte);
    let h = commitment(&f.env, value, &nonce);
    f.client.commit(
        &round_id,
        bidder,
        &h,
        &Bytes::from_array(&f.env, b"sealed"),
        &escrow,
        &Bytes::from_array(&f.env, b"id-blob"),
    );
    nonce
}

fn assert_try_contract_err<T>(
    result: Result<Result<T, ConversionError>, Result<Error, InvokeError>>,
    expected: Error,
) {
    match result {
        Err(Ok(got)) => {
            assert_eq!(
                got, expected,
                "expected {} ({})",
                variant_name(expected),
                expected as u32,
            );
        }
        Err(Err(InvokeError::Contract(code))) => {
            assert_eq!(
                code,
                expected as u32,
                "expected {} ({})",
                variant_name(expected),
                expected as u32,
            );
        }
        Err(Err(other)) => panic!(
            "expected {} ({}), got invoke error {:?}",
            variant_name(expected),
            expected as u32,
            other,
        ),
        Ok(Ok(_)) => panic!(
            "expected contract error {} ({}), call succeeded",
            variant_name(expected),
            expected as u32,
        ),
        Ok(Err(conv)) => panic!("argument conversion error: {:?}", conv),
    }
}

fn assert_try_create_round_err(
    result: Result<
        Result<u64, soroban_sdk::Error>,
        Result<Error, InvokeError>,
    >,
    expected: Error,
) {
    match result {
        Err(Ok(got)) => {
            assert_eq!(
                got, expected,
                "expected {} ({})",
                variant_name(expected),
                expected as u32,
            );
        }
        Err(Err(InvokeError::Contract(code))) => {
            assert_eq!(
                code,
                expected as u32,
                "expected {} ({})",
                variant_name(expected),
                expected as u32,
            );
        }
        Err(Err(other)) => panic!(
            "expected {} ({}), got invoke error {:?}",
            variant_name(expected),
            expected as u32,
            other,
        ),
        Ok(Ok(_)) => panic!(
            "expected contract error {} ({}), call succeeded",
            variant_name(expected),
            expected as u32,
        ),
        Ok(Err(conv)) => panic!("return value conversion error: {:?}", conv),
    }
}

#[path = "error_paths.rs"]
mod error_paths;

// ─────────────────────────────────────────────────────────────────────────────
// EXISTING TESTS (preserved verbatim)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn create_round_happy_path() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    assert_eq!(id, 1);
    let round = f.client.get_round(&id);
    assert_eq!(round.operator, operator);
    assert_eq!(round.reveal_round, 2_000);
    assert_eq!(round.bidders.len(), 0);
}

#[test]
fn create_round_rejects_commit_after_reveal() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let res = f.client.try_create_round(
        &operator, &b32(&f.env, 1), &2_000, &ClearingRule::HighestBid,
        &2_000, &2_500, &Bytes::from_array(&f.env, b"a"),
    );
    assert!(res.is_err());
}

#[test]
fn create_round_rejects_deadline_in_past() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let res = f.client.try_create_round(
        &operator, &b32(&f.env, 1), &2_000, &ClearingRule::HighestBid,
        &500, &2_500, &Bytes::from_array(&f.env, b"a"),
    );
    assert!(res.is_err());
}

#[test]
fn commit_locks_escrow() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let bidder = funded_bidder(&f, 1_000);
    f.client.commit(&id, &bidder, &b32(&f.env, 7), &Bytes::from_array(&f.env, b"ciphertext"), &600, &Bytes::from_array(&f.env, b"id-blob"));
    assert_eq!(f.usdc_token.balance(&bidder), 400);
    assert_eq!(f.usdc_token.balance(&f.client.address), 600);
    let round = f.client.get_round(&id);
    assert_eq!(round.bidders.len(), 1);
    let state = f.client.get_bid_state(&id, &bidder);
    assert_eq!(state.escrow, 600);
    assert_eq!(state.valid, false);
}

#[test]
fn get_bidders_returns_ordered_index() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let a = funded_bidder(&f, 1_000);
    let b = funded_bidder(&f, 1_000);
    f.client.commit(&id, &a, &b32(&f.env, 1), &Bytes::from_array(&f.env, b"c"), &100, &Bytes::from_array(&f.env, b"id"));
    f.client.commit(&id, &b, &b32(&f.env, 2), &Bytes::from_array(&f.env, b"c"), &200, &Bytes::from_array(&f.env, b"id"));
    f.client.commit(&id, &a, &b32(&f.env, 3), &Bytes::from_array(&f.env, b"c"), &150, &Bytes::from_array(&f.env, b"id"));
    let bidders = f.client.get_bidders(&id);
    assert_eq!(bidders.len(), 2);
    assert_eq!(bidders.get(0).unwrap(), a);
    assert_eq!(bidders.get(1).unwrap(), b);
}

#[test]
fn commit_overwrite_before_close_refunds_prior_escrow() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let bidder = funded_bidder(&f, 1_000);
    f.client.commit(&id, &bidder, &b32(&f.env, 7), &Bytes::from_array(&f.env, b"c1"), &600, &Bytes::from_array(&f.env, b"id"));
    f.client.commit(&id, &bidder, &b32(&f.env, 9), &Bytes::from_array(&f.env, b"c2"), &200, &Bytes::from_array(&f.env, b"id"));
    assert_eq!(f.usdc_token.balance(&bidder), 800);
    assert_eq!(f.usdc_token.balance(&f.client.address), 200);
    assert_eq!(f.client.get_round(&id).bidders.len(), 1);
}

#[test]
fn commit_after_deadline_rejected() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let bidder = funded_bidder(&f, 1_000);
    f.env.ledger().with_mut(|l| l.timestamp = 1_600);
    let res = f.client.try_commit(&id, &bidder, &b32(&f.env, 7), &Bytes::from_array(&f.env, b"c"), &600, &Bytes::from_array(&f.env, b"id"));
    assert!(res.is_err());
}

#[test]
fn commit_zero_escrow_rejected() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let bidder = funded_bidder(&f, 1_000);
    let res = f.client.try_commit(&id, &bidder, &b32(&f.env, 7), &Bytes::from_array(&f.env, b"c"), &0, &Bytes::from_array(&f.env, b"id"));
    assert!(res.is_err());
}

#[test]
fn void_after_grace_refunds_all() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let a = funded_bidder(&f, 1_000);
    let bbidder = funded_bidder(&f, 1_000);
    f.client.commit(&id, &a, &b32(&f.env, 1), &Bytes::from_array(&f.env, b"c"), &300, &Bytes::from_array(&f.env, b"id"));
    f.client.commit(&id, &bbidder, &b32(&f.env, 2), &Bytes::from_array(&f.env, b"c"), &500, &Bytes::from_array(&f.env, b"id"));
    f.env.ledger().with_mut(|l| l.timestamp = 2_500 + 3_600 + 1);
    f.client.void(&id);
    assert_eq!(f.usdc_token.balance(&a), 1_000);
    assert_eq!(f.usdc_token.balance(&bbidder), 1_000);
    assert_eq!(f.client.get_round(&id).bidders.len(), 2);
}

// ─────────────────────────────────────────────────────────────────────────────
// ISSUE #6 — NEW TESTS (all reveal-path tests use setup_drand + real signature)
// ─────────────────────────────────────────────────────────────────────────────

// ── 1. HighestBid and LowestBid, including deterministic ties ────────────────

#[test]
fn highest_bid_table_driven() {
    struct Case { bids: &'static [i128], expected_winner_idx: usize }
    let cases = [
        Case { bids: &[100, 200, 300], expected_winner_idx: 2 },
        Case { bids: &[999, 1, 500],   expected_winner_idx: 0 },
        Case { bids: &[50, 50, 51],    expected_winner_idx: 2 },
        Case { bids: &[1],             expected_winner_idx: 0 },
    ];

    for case in &cases {
        let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
        let operator = Address::generate(&f.env);
        let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

        let mut bidders = Vec::new(&f.env);
        let mut nonces: Vec<BytesN<32>> = Vec::new(&f.env);
        for (i, &value) in case.bids.iter().enumerate() {
            let bidder = funded_bidder(&f, value + 100);
            let nonce = commit_bid(&f, id, &bidder, value, value, (i + 1) as u8);
            bidders.push_back(bidder);
            nonces.push_back(nonce);
        }

        f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
        f.client.open_reveal(&id, &real_sig(&f.env));

        for i in 0..case.bids.len() {
            f.client.reveal(&id, &bidders.get(i as u32).unwrap(), &case.bids[i], &nonces.get(i as u32).unwrap());
        }

        f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
        let winner = f.client.clear(&id);
        assert_eq!(winner, Some(bidders.get(case.expected_winner_idx as u32).unwrap()),
            "HighestBid {:?}: expected winner index {}", case.bids, case.expected_winner_idx);
    }
}

#[test]
fn lowest_bid_table_driven() {
    struct Case { bids: &'static [i128], expected_winner_idx: usize }
    let cases = [
        Case { bids: &[300, 200, 100], expected_winner_idx: 2 },
        Case { bids: &[1, 999, 500],   expected_winner_idx: 0 },
        Case { bids: &[51, 50, 50],    expected_winner_idx: 1 },
    ];

    for case in &cases {
        let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
        let operator = Address::generate(&f.env);
        let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::LowestBid);

        let mut bidders = Vec::new(&f.env);
        let mut nonces: Vec<BytesN<32>> = Vec::new(&f.env);
        for (i, &value) in case.bids.iter().enumerate() {
            let bidder = funded_bidder(&f, 2_000);
            let nonce = commit_bid(&f, id, &bidder, value, 2_000, (i + 10) as u8);
            bidders.push_back(bidder);
            nonces.push_back(nonce);
        }

        f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
        f.client.open_reveal(&id, &real_sig(&f.env));

        for i in 0..case.bids.len() {
            f.client.reveal(&id, &bidders.get(i as u32).unwrap(), &case.bids[i], &nonces.get(i as u32).unwrap());
        }

        f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
        let winner = f.client.clear(&id);
        assert_eq!(winner, Some(bidders.get(case.expected_winner_idx as u32).unwrap()),
            "LowestBid {:?}: expected winner index {}", case.bids, case.expected_winner_idx);
    }
}

#[test]
fn highest_bid_tie_is_deterministic_first_inserter_wins() {
    let tied_value: i128 = 500;

    // Alice commits first -> should win the tie
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);
    let alice = funded_bidder(&f, 1_000);
    let bob   = funded_bidder(&f, 1_000);
    let a_nonce = commit_bid(&f, id, &alice, tied_value, 500, 0xAA);
    let b_nonce = commit_bid(&f, id, &bob,   tied_value, 500, 0xBB);
    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    f.client.reveal(&id, &alice, &tied_value, &a_nonce);
    f.client.reveal(&id, &bob,   &tied_value, &b_nonce);
    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    assert_eq!(f.client.clear(&id), Some(alice.clone()), "tie: first inserter (alice) must win");

    // Reversed insertion order: bob commits first -> bob should win
    let (f2, t2, cd2, rd2) = setup_drand();
    let op2   = Address::generate(&f2.env);
    let id2   = drand_round(&f2, &op2, cd2, rd2, ClearingRule::HighestBid);
    let bob2   = funded_bidder(&f2, 1_000);
    let alice2 = funded_bidder(&f2, 1_000);
    let b2_nonce = commit_bid(&f2, id2, &bob2,   tied_value, 500, 0xBB);
    let a2_nonce = commit_bid(&f2, id2, &alice2, tied_value, 500, 0xAA);
    f2.env.ledger().with_mut(|l| l.timestamp = t2 + 1);
    f2.client.open_reveal(&id2, &real_sig(&f2.env));
    f2.client.reveal(&id2, &bob2,   &tied_value, &b2_nonce);
    f2.client.reveal(&id2, &alice2, &tied_value, &a2_nonce);
    f2.env.ledger().with_mut(|l| l.timestamp = rd2 + 1);
    assert_eq!(f2.client.clear(&id2), Some(bob2.clone()), "tie reversed: bob must win");
}

#[test]
fn lowest_bid_tie_is_deterministic_first_inserter_wins() {
    let tied_value: i128 = 200;
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::LowestBid);
    let alice = funded_bidder(&f, 1_000);
    let bob   = funded_bidder(&f, 1_000);
    let a_nonce = commit_bid(&f, id, &alice, tied_value, 1_000, 0xCC);
    let b_nonce = commit_bid(&f, id, &bob,   tied_value, 1_000, 0xDD);
    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    f.client.reveal(&id, &alice, &tied_value, &a_nonce);
    f.client.reveal(&id, &bob,   &tied_value, &b_nonce);
    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    assert_eq!(f.client.clear(&id), Some(alice.clone()), "LowestBid tie: first inserter wins");
}

// ── 2. Mixed reveals: valid, invalid, missing, duplicate ─────────────────────

#[test]
fn reveal_hash_mismatch_invalidates_bid() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

    let good = funded_bidder(&f, 1_000);
    let bad  = funded_bidder(&f, 1_000);
    let good_nonce = commit_bid(&f, id, &good, 300, 300, 0x01);
    let bad_nonce  = commit_bid(&f, id, &bad,  500, 500, 0x02);

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    f.client.reveal(&id, &good, &300, &good_nonce);
    // Wrong value -> hash mismatch -> rejected
    assert!(f.client.try_reveal(&id, &bad, &999, &bad_nonce).is_err(), "hash mismatch must be rejected");

    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    assert_eq!(f.client.clear(&id), Some(good.clone()), "only valid revealer must win");
}

#[test]
fn missing_reveal_bid_is_skipped_during_clear() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

    let revealer = funded_bidder(&f, 1_000);
    let ghoster  = funded_bidder(&f, 1_000);
    let r_nonce  = commit_bid(&f, id, &revealer, 100, 100, 0x01);
    let _        = commit_bid(&f, id, &ghoster,  999, 999, 0x02); // never reveals

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    f.client.reveal(&id, &revealer, &100, &r_nonce);

    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    assert_eq!(f.client.clear(&id), Some(revealer.clone()), "unrevealed high bid must be skipped");
}

#[test]
fn duplicate_reveal_rejected() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

    let bidder = funded_bidder(&f, 1_000);
    let nonce  = commit_bid(&f, id, &bidder, 400, 400, 0x05);

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    f.client.reveal(&id, &bidder, &400, &nonce);
    assert!(f.client.try_reveal(&id, &bidder, &400, &nonce).is_err(), "duplicate reveal must be rejected");
}

#[test]
fn reveal_wrong_nonce_rejected() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

    let bidder = funded_bidder(&f, 1_000);
    let _correct_nonce = commit_bid(&f, id, &bidder, 400, 400, 0x07);
    let wrong_nonce    = b32(&f.env, 0x99);

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    assert!(f.client.try_reveal(&id, &bidder, &400, &wrong_nonce).is_err(), "wrong nonce must be rejected");
}

#[test]
fn no_valid_bids_after_reveal_window() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

    let bidder = funded_bidder(&f, 1_000);
    let _ = commit_bid(&f, id, &bidder, 500, 500, 0x01); // never reveals

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));

    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    assert_eq!(f.client.clear(&id), None, "no valid bids -> winner must be None");
}

// ── 3. Repeated pre-deadline overwrites with changing escrow ──────────────────

#[test]
fn repeated_overwrites_escrow_conservation() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let bidder = funded_bidder(&f, 2_000);
    let initial: i128 = 2_000;
    let escrows: &[i128] = &[500, 300, 800, 100];
    for (i, &escrow) in escrows.iter().enumerate() {
        f.client.commit(&id, &bidder, &b32(&f.env, (i + 1) as u8), &Bytes::from_array(&f.env, b"c"), &escrow, &Bytes::from_array(&f.env, b"id"));
        let sum = f.usdc_token.balance(&bidder) + f.usdc_token.balance(&f.client.address);
        assert_eq!(sum, initial, "conservation violated after overwrite #{}", i + 1);
        assert_eq!(f.usdc_token.balance(&f.client.address), escrow, "contract must hold latest escrow after #{}", i + 1);
    }
    assert_eq!(f.client.get_round(&id).bidders.len(), 1);
}

#[test]
fn overwrite_to_larger_escrow_conserves_tokens() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let bidder = funded_bidder(&f, 1_000);
    f.client.commit(&id, &bidder, &b32(&f.env, 1), &Bytes::from_array(&f.env, b"c"), &200, &Bytes::from_array(&f.env, b"id"));
    f.client.commit(&id, &bidder, &b32(&f.env, 2), &Bytes::from_array(&f.env, b"c"), &700, &Bytes::from_array(&f.env, b"id"));
    assert_eq!(f.usdc_token.balance(&bidder), 300);
    assert_eq!(f.usdc_token.balance(&f.client.address), 700);
    assert_eq!(f.usdc_token.balance(&bidder) + f.usdc_token.balance(&f.client.address), 1_000);
}

// ── 4. Token conservation after every escrow-changing action ─────────────────

#[test]
fn token_conservation_full_lifecycle() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

    let alice = funded_bidder(&f, 1_000);
    let bob   = funded_bidder(&f, 1_000);
    let total: i128 = 2_000;

    let check = |label: &str| {
        let sum = f.usdc_token.balance(&alice)
            + f.usdc_token.balance(&bob)
            + f.usdc_token.balance(&operator)
            + f.usdc_token.balance(&f.client.address);
        assert_eq!(sum, total, "conservation violated at: {}", label);
    };

    let a_nonce = commit_bid(&f, id, &alice, 700, 700, 0x11);
    check("after alice commit");
    let b_nonce = commit_bid(&f, id, &bob, 500, 500, 0x22);
    check("after bob commit");

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    check("after open_reveal");

    f.client.reveal(&id, &alice, &700, &a_nonce);
    check("after alice reveal");
    f.client.reveal(&id, &bob, &500, &b_nonce);
    check("after bob reveal");

    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    f.client.clear(&id);
    check("after clear");

    f.client.settle(&id);
    check("after settle");
}

// ── 5. Zero contract balance after settlement or void ────────────────────────

#[test]
fn contract_balance_zero_after_settle() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

    let alice = funded_bidder(&f, 1_000);
    let bob   = funded_bidder(&f, 1_000);
    let a_nonce = commit_bid(&f, id, &alice, 700, 700, 0x11);
    let b_nonce = commit_bid(&f, id, &bob,   500, 500, 0x22);

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    f.client.reveal(&id, &alice, &700, &a_nonce);
    f.client.reveal(&id, &bob,   &500, &b_nonce);
    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    f.client.clear(&id);
    f.client.settle(&id);

    assert_eq!(f.usdc_token.balance(&f.client.address), 0, "contract must hold zero after settle");
}

#[test]
fn contract_balance_zero_after_void() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let alice = funded_bidder(&f, 1_000);
    let bob   = funded_bidder(&f, 1_000);
    commit_bid(&f, id, &alice, 700, 700, 0x11);
    commit_bid(&f, id, &bob,   500, 500, 0x22);
    f.env.ledger().with_mut(|l| l.timestamp = 2_500 + 3_600 + 1);
    f.client.void(&id);
    assert_eq!(f.usdc_token.balance(&f.client.address), 0, "contract must hold zero after void");
}

// ── 6. Operator/winner/refund amounts are exact and paid once ─────────────────

#[test]
fn settle_exact_payouts_table() {
    struct Case {
        bids:        &'static [i128],
        escrows:     &'static [i128],
        winning_idx: usize,
        op_gets:     i128,
        winner_surplus: i128,
        loser_refunds: &'static [i128],
    }
    let cases = [
        Case { bids: &[700, 500], escrows: &[700, 500], winning_idx: 0, op_gets: 700, winner_surplus: 0,   loser_refunds: &[500] },
        Case { bids: &[700, 500], escrows: &[1_000, 800], winning_idx: 0, op_gets: 700, winner_surplus: 300, loser_refunds: &[800] },
        Case { bids: &[100, 200, 150], escrows: &[100, 200, 150], winning_idx: 1, op_gets: 200, winner_surplus: 0, loser_refunds: &[100, 150] },
    ];

    for (ci, case) in cases.iter().enumerate() {
        let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
        let operator = Address::generate(&f.env);
        let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

        let mut bidders = Vec::new(&f.env);
        let mut nonces: Vec<BytesN<32>> = Vec::new(&f.env);
        for (i, (&bid, &escrow)) in case.bids.iter().zip(case.escrows.iter()).enumerate() {
            let bidder = funded_bidder(&f, escrow);
            let nonce = commit_bid(&f, id, &bidder, bid, escrow, (i + 1) as u8);
            bidders.push_back(bidder);
            nonces.push_back(nonce);
        }

        f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
        f.client.open_reveal(&id, &real_sig(&f.env));
        for i in 0..case.bids.len() {
            f.client.reveal(&id, &bidders.get(i as u32).unwrap(), &case.bids[i], &nonces.get(i as u32).unwrap());
        }
        f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
        f.client.clear(&id);
        f.client.settle(&id);

        let winner = bidders.get(case.winning_idx as u32).unwrap();
        assert_eq!(f.usdc_token.balance(&operator), case.op_gets, "case {}: operator payout", ci);
        assert_eq!(f.usdc_token.balance(&winner), case.winner_surplus, "case {}: winner surplus", ci);
        for (li, &expected) in case.loser_refunds.iter().enumerate() {
            let idx = if li < case.winning_idx { li } else { li + 1 };
            let loser = bidders.get(idx as u32).unwrap();
            assert_eq!(f.usdc_token.balance(&loser), expected, "case {}: loser {} refund", ci, li);
        }
        assert_eq!(f.usdc_token.balance(&f.client.address), 0, "case {}: contract not drained", ci);
    }
}

// ── 7. Terminal states cannot go backward or move funds twice ─────────────────

#[test]
fn double_settle_rejected() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

    let alice = funded_bidder(&f, 1_000);
    let a_nonce = commit_bid(&f, id, &alice, 500, 500, 0x01);
    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    f.client.reveal(&id, &alice, &500, &a_nonce);
    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    f.client.clear(&id);
    f.client.settle(&id);

    assert!(f.client.try_settle(&id).is_err(), "double settle must be rejected");
    assert_eq!(f.usdc_token.balance(&operator), 500, "operator must not receive funds twice");
}

#[test]
fn double_void_rejected() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let alice = funded_bidder(&f, 500);
    commit_bid(&f, id, &alice, 500, 500, 0x01);
    f.env.ledger().with_mut(|l| l.timestamp = 2_500 + 3_600 + 1);
    f.client.void(&id);
    assert!(f.client.try_void(&id).is_err(), "double void must be rejected");
    assert_eq!(f.usdc_token.balance(&alice), 500, "alice must not be refunded twice");
}

#[test]
fn commit_on_settled_round_rejected() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

    let alice = funded_bidder(&f, 1_000);
    let a_nonce = commit_bid(&f, id, &alice, 500, 500, 0x01);
    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    f.client.reveal(&id, &alice, &500, &a_nonce);
    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    f.client.clear(&id);
    f.client.settle(&id);

    let late = funded_bidder(&f, 1_000);
    assert!(f.client.try_commit(&id, &late, &b32(&f.env, 0x77), &Bytes::from_array(&f.env, b"c"), &100, &Bytes::from_array(&f.env, b"id")).is_err(),
        "commit on settled round must be rejected");
}

#[test]
fn open_reveal_on_settled_round_rejected() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

    let alice = funded_bidder(&f, 1_000);
    let a_nonce = commit_bid(&f, id, &alice, 500, 500, 0x01);
    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    f.client.reveal(&id, &alice, &500, &a_nonce);
    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    f.client.clear(&id);
    f.client.settle(&id);

    assert!(f.client.try_open_reveal(&id, &real_sig(&f.env)).is_err(),
        "open_reveal on settled round must be rejected");
}

#[test]
fn reveal_on_settled_round_rejected() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

    let alice = funded_bidder(&f, 1_000);
    let bob   = funded_bidder(&f, 1_000);
    let a_nonce = commit_bid(&f, id, &alice, 700, 700, 0x01);
    let b_nonce = commit_bid(&f, id, &bob,   300, 300, 0x02);

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    f.client.reveal(&id, &alice, &700, &a_nonce);
    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    f.client.clear(&id);
    f.client.settle(&id);

    assert!(f.client.try_reveal(&id, &bob, &300, &b_nonce).is_err(),
        "reveal on settled round must be rejected");
}

// ── 8. Generated cases reproducible from a printed seed ──────────────────────

#[test]
fn seeded_case_42_highest_bid_reproducible() {
    // Seed 42 -> bids [142, 242, 92], escrows [200, 300, 150]; winner index 1 (bid 242)
    let bids:    &[i128] = &[142, 242, 92];
    let escrows: &[i128] = &[200, 300, 150];

    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);

    let mut bidders = Vec::new(&f.env);
    let mut nonces: Vec<BytesN<32>> = Vec::new(&f.env);
    for (i, (&bid, &escrow)) in bids.iter().zip(escrows.iter()).enumerate() {
        let bidder = funded_bidder(&f, escrow);
        let nonce = commit_bid(&f, id, &bidder, bid, escrow, (42 + i) as u8);
        bidders.push_back(bidder);
        nonces.push_back(nonce);
    }

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    for i in 0..bids.len() {
        f.client.reveal(&id, &bidders.get(i as u32).unwrap(), &bids[i], &nonces.get(i as u32).unwrap());
    }
    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    let winner = f.client.clear(&id);
    assert_eq!(winner, Some(bidders.get(1).unwrap()), "seed-42: winner must be index 1 (bid=242)");
    assert_eq!(f.client.get_round(&id).winning_bid, 242);
}

#[test]
fn seeded_case_7_lowest_bid_reproducible() {
    // Seed 7 -> bids [107, 207, 57], escrows [500, 500, 500]; winner index 2 (bid 57)
    let bids:    &[i128] = &[107, 207, 57];
    let escrows: &[i128] = &[500, 500, 500];

    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::LowestBid);

    let mut bidders = Vec::new(&f.env);
    let mut nonces: Vec<BytesN<32>> = Vec::new(&f.env);
    for (i, (&bid, &escrow)) in bids.iter().zip(escrows.iter()).enumerate() {
        let bidder = funded_bidder(&f, escrow);
        let nonce = commit_bid(&f, id, &bidder, bid, escrow, (7 + i) as u8);
        bidders.push_back(bidder);
        nonces.push_back(nonce);
    }

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    for i in 0..bids.len() {
        f.client.reveal(&id, &bidders.get(i as u32).unwrap(), &bids[i], &nonces.get(i as u32).unwrap());
    }
    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    let winner = f.client.clear(&id);
    assert_eq!(winner, Some(bidders.get(2).unwrap()), "seed-7: winner must be index 2 (bid=57)");
    assert_eq!(f.client.get_round(&id).winning_bid, 57);
}

// ─────────────────────────────────────────────────────────────────────────────
// REAL DRAND VECTOR TESTS (preserved verbatim)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn drand_bls_verify_real_vector() {
    let env = Env::default();
    let sig = hexn::<96>(&env, VEC_SIG_G1);
    let cfg = config_with(&env, VEC_PUBKEY_C1C0, VEC_NEGGEN_C1C0);
    assert!(drand::verify_round(&env, &cfg, VEC_ROUND, &sig),
        "c1c0-ordered constants must verify the live quicknet signature on-chain");
}

#[test]
fn drand_bls_verify_rejects_wrong_round() {
    let env = Env::default();
    let sig = hexn::<96>(&env, VEC_SIG_G1);
    let cfg = config_with(&env, VEC_PUBKEY_C1C0, VEC_NEGGEN_C1C0);
    assert!(!drand::verify_round(&env, &cfg, VEC_ROUND + 1, &sig));
}

fn setup_real_drand() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();
    let issuer = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(issuer);
    let usdc = sac.address();
    let contract_id = env.register(
        SubRosaRound,
        (
            hexn::<192>(&env, VEC_PUBKEY_C1C0),
            hexn::<192>(&env, VEC_NEGGEN_C1C0),
            Bytes::from_slice(&env, VEC_DST),
            VEC_GENESIS,
            VEC_PERIOD,
            usdc.clone(),
        ),
    );
    let client = SubRosaRoundClient::new(&env, &contract_id);
    Fixture {
        env: env.clone(),
        client,
        usdc_admin: token::StellarAssetClient::new(&env, &usdc),
        usdc_token: token::Client::new(&env, &usdc),
    }
}

#[test]
fn full_lifecycle_real_drand_signature() {
    let f = setup_real_drand();
    let t_reveal = VEC_GENESIS + VEC_PERIOD * VEC_ROUND;
    let commit_deadline = t_reveal - 10;
    let reveal_deadline = t_reveal + 100;
    f.env.ledger().with_mut(|l| l.timestamp = t_reveal - 100);

    let operator = Address::generate(&f.env);
    let id = f.client.create_round(
        &operator, &b32(&f.env, 0xAB), &VEC_ROUND, &ClearingRule::HighestBid,
        &commit_deadline, &reveal_deadline, &Bytes::from_array(&f.env, b"auditor"),
    );

    let alice = funded_bidder(&f, 1_000);
    let bob   = funded_bidder(&f, 1_000);
    let a_nonce = b32(&f.env, 0x11);
    let b_nonce = b32(&f.env, 0x22);
    let a_value: i128 = 700;
    let b_value: i128 = 500;

    f.client.commit(&id, &alice, &commitment(&f.env, a_value, &a_nonce), &Bytes::from_array(&f.env, b"sealedA"), &1_000, &Bytes::from_array(&f.env, b"idA"));
    f.client.commit(&id, &bob,   &commitment(&f.env, b_value, &b_nonce), &Bytes::from_array(&f.env, b"sealedB"), &1_000, &Bytes::from_array(&f.env, b"idB"));

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    let sig = hexn::<96>(&f.env, VEC_SIG_G1);
    f.client.open_reveal(&id, &sig);
    assert_eq!(f.client.get_round(&id).status, Status::Revealing);

    assert!(f.client.try_reveal(&id, &alice, &a_value, &b32(&f.env, 0x99)).is_err());
    f.client.reveal(&id, &alice, &a_value, &a_nonce);
    f.client.reveal(&id, &bob,   &b_value, &b_nonce);

    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    assert_eq!(f.client.clear(&id), Some(alice.clone()));
    assert_eq!(f.client.get_round(&id).winning_bid, 700);

    f.client.settle(&id);
    assert_eq!(f.usdc_token.balance(&operator), 700);
    assert_eq!(f.usdc_token.balance(&alice), 300);
    assert_eq!(f.usdc_token.balance(&bob), 1_000);
    assert_eq!(f.usdc_token.balance(&f.client.address), 0);
    assert_eq!(f.client.get_round(&id).status, Status::Settled);
}

#[test]
fn commitment_matches_offchain_vector() {
    let env = Env::default();
    let h = commitment(&env, 700, &b32(&env, 0x11));
    let expected = hexn::<32>(&env, "3d4c2d3604b23250687f0344a9474e3c748742a4fba4616d308d529121a8dec4");
    assert_eq!(h, expected);
}

// ── Paginated get_bidders_page (preserved verbatim) ──────────────────────────

fn round_with_n_bidders(n: u32) -> (Fixture, u64, Vec<Address>) {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let mut all = Vec::new(&f.env);
    for i in 0..n {
        let bidder = funded_bidder(&f, 1_000 + i as i128);
        f.client.commit(&id, &bidder, &b32(&f.env, (i + 1) as u8), &Bytes::from_array(&f.env, b"c"), &100, &Bytes::from_array(&f.env, b"id"));
        all.push_back(bidder);
    }
    (f, id, all)
}

#[test]
fn get_bidders_page_empty() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let page = f.client.get_bidders_page(&id, &0, &10);
    assert_eq!(page.data.len(), 0);
    assert_eq!(page.next_cursor, 0);
    assert_eq!(page.total, 0);
}

#[test]
fn get_bidders_page_partial() {
    let (f, id, _) = round_with_n_bidders(5);
    let page = f.client.get_bidders_page(&id, &0, &3);
    assert_eq!(page.data.len(), 3);
    assert_eq!(page.next_cursor, 3);
    assert_eq!(page.total, 5);
}

#[test]
fn get_bidders_page_exact() {
    let (f, id, _) = round_with_n_bidders(3);
    let page = f.client.get_bidders_page(&id, &0, &3);
    assert_eq!(page.data.len(), 3);
    assert_eq!(page.next_cursor, 0);
    assert_eq!(page.total, 3);
}

#[test]
fn get_bidders_page_final() {
    let (f, id, _) = round_with_n_bidders(5);
    let page = f.client.get_bidders_page(&id, &3, &3);
    assert_eq!(page.data.len(), 2);
    assert_eq!(page.next_cursor, 0);
    assert_eq!(page.total, 5);
}

#[test]
fn get_bidders_page_multi() {
    let (f, id, all) = round_with_n_bidders(10);
    let p1 = f.client.get_bidders_page(&id, &0, &4);
    assert_eq!(p1.data.len(), 4); assert_eq!(p1.next_cursor, 4); assert_eq!(p1.total, 10);
    assert_eq!(p1.data.get(0).unwrap(), all.get(0).unwrap());
    assert_eq!(p1.data.get(3).unwrap(), all.get(3).unwrap());
    let p2 = f.client.get_bidders_page(&id, &p1.next_cursor, &4);
    assert_eq!(p2.data.len(), 4); assert_eq!(p2.next_cursor, 8);
    assert_eq!(p2.data.get(0).unwrap(), all.get(4).unwrap());
    assert_eq!(p2.data.get(3).unwrap(), all.get(7).unwrap());
    let p3 = f.client.get_bidders_page(&id, &p2.next_cursor, &4);
    assert_eq!(p3.data.len(), 2); assert_eq!(p3.next_cursor, 0);
    assert_eq!(p3.data.get(0).unwrap(), all.get(8).unwrap());
    assert_eq!(p3.data.get(1).unwrap(), all.get(9).unwrap());
}

#[test]
fn get_bidders_page_rejects_limit_zero() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    assert!(f.client.try_get_bidders_page(&id, &0, &0).is_err());
}

#[test]
fn get_bidders_page_rejects_limit_over_max() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    assert!(f.client.try_get_bidders_page(&id, &0, &101).is_err());
}

#[test]
fn get_bidders_page_cursor_at_total() {
    let (f, id, _) = round_with_n_bidders(3);
    let page = f.client.get_bidders_page(&id, &3, &5);
    assert_eq!(page.data.len(), 0); assert_eq!(page.next_cursor, 0); assert_eq!(page.total, 3);
}

#[test]
fn get_bidders_page_cursor_beyond_total() {
    let (f, id, _) = round_with_n_bidders(3);
    let page = f.client.get_bidders_page(&id, &10, &5);
    assert_eq!(page.data.len(), 0); assert_eq!(page.next_cursor, 0); assert_eq!(page.total, 3);
}

#[test]
fn get_bidders_page_preserves_order() {
    let (f, id, all) = round_with_n_bidders(5);
    let mut collected = Vec::new(&f.env);
    let mut cursor: u32 = 0;
    loop {
        let page = f.client.get_bidders_page(&id, &cursor, &2);
        for i in 0..page.data.len() { collected.push_back(page.data.get(i).unwrap()); }
        if page.next_cursor == 0 { break; }
        cursor = page.next_cursor;
    }
    assert_eq!(collected.len(), 5);
    for i in 0..5 { assert_eq!(collected.get(i).unwrap(), all.get(i).unwrap()); }
}

#[test]
fn get_bidders_still_returns_full_list() {
    let (f, id, all) = round_with_n_bidders(5);
    let full = f.client.get_bidders(&id);
    assert_eq!(full.len(), 5);
    for i in 0..5 { assert_eq!(full.get(i).unwrap(), all.get(i).unwrap()); }
}

#[test]
fn void_before_grace_rejected() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    f.env.ledger().with_mut(|l| l.timestamp = 2_600);
    assert!(f.client.try_void(&id).is_err());
}

// ── Storage expiration and cleanup coverage (#51) ────────────────────────────

fn seal_key(round_id: u64, bidder: &Address) -> DataKey {
    DataKey::Seal(round_id, bidder.clone())
}

fn temporary_seal_ttl(f: &Fixture, key: &DataKey) -> u32 {
    f.env.as_contract(&f.client.address, || {
        f.env.storage().temporary().get_ttl(key)
    })
}

fn advance_ledgers(f: &Fixture, count: u32) {
    let seq = f.env.ledger().sequence();
    f.env.ledger().set_sequence_number(seq + count);
}

fn active_round_with_bidder() -> (Fixture, u64, Address, u64) {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let bidder = funded_bidder(&f, 1_000);
    commit_bid(&f, id, &bidder, 500, 1_000, 0x01);
    let reveal_deadline = f.client.get_round(&id).reveal_deadline;
    (f, id, bidder, reveal_deadline)
}

#[test]
fn active_round_seal_ttl_covers_reveal_window() {
    let (f, id, bidder, reveal_deadline) = active_round_with_bidder();
    let now = f.env.ledger().timestamp();
    let expected = seal_ttl_for_reveal_deadline(reveal_deadline, now);
    let ttl = temporary_seal_ttl(&f, &seal_key(id, &bidder));
    assert!(
        ttl >= expected.saturating_sub(1),
        "seal TTL {ttl} should cover reveal window ({expected} ledgers)"
    );
    assert!(ttl >= TEMP_THRESHOLD);
    assert!(f.client.get_seal(&id, &bidder).is_some());
}

#[test]
fn open_reveal_extends_seal_through_reveal_window() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);
    let bidder = funded_bidder(&f, 1_000);
    commit_bid(&f, id, &bidder, 500, 1_000, 0x01);

    advance_ledgers(&f, TEMP_THRESHOLD);
    let ttl_before = temporary_seal_ttl(&f, &seal_key(id, &bidder));

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));

    let ttl_after = temporary_seal_ttl(&f, &seal_key(id, &bidder));
    let expected = seal_ttl_for_reveal_deadline(reveal_deadline, f.env.ledger().timestamp());
    assert!(
        ttl_after >= expected.saturating_sub(1),
        "open_reveal should re-extend seal TTL to {expected}, got {ttl_after}"
    );
    assert!(ttl_after >= ttl_before);
    assert!(f.client.get_seal(&id, &bidder).is_some());
}

#[test]
fn settled_round_persistent_state_survives_seal_expiry() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);
    let alice = funded_bidder(&f, 1_000);
    let nonce = commit_bid(&f, id, &alice, 700, 1_000, 0x11);

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    f.client.reveal(&id, &alice, &700, &nonce);
    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    f.client.clear(&id);
    f.client.settle(&id);

    let ttl = temporary_seal_ttl(&f, &seal_key(id, &alice));
    advance_ledgers(&f, ttl + 1);
    assert!(f.client.get_seal(&id, &alice).is_none(), "seal should expire after TTL");
    assert_eq!(f.client.get_round(&id).status, Status::Settled);
    assert_eq!(f.client.get_bid_state(&id, &alice).revealed_value, Some(700));
    assert_eq!(f.usdc_token.balance(&f.client.address), 0);
}

#[test]
fn voided_round_refunds_survive_seal_expiry() {
    let f = setup();
    let operator = Address::generate(&f.env);
    let id = open_round(&f, &operator);
    let alice = funded_bidder(&f, 1_000);
    commit_bid(&f, id, &alice, 500, 1_000, 0x01);

    let ttl = temporary_seal_ttl(&f, &seal_key(id, &alice));
    advance_ledgers(&f, ttl + 1);
    assert!(f.client.get_seal(&id, &alice).is_none());

    f.env.ledger().with_mut(|l| l.timestamp = 2_500 + 3_601);
    f.client.void(&id);
    assert_eq!(f.client.get_round(&id).status, Status::Voided);
    assert_eq!(f.usdc_token.balance(&alice), 1_000);
    assert_eq!(f.usdc_token.balance(&f.client.address), 0);
}

#[test]
fn late_reveal_rejected_after_window_even_with_seal_present() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);
    let alice = funded_bidder(&f, 1_000);
    let nonce = commit_bid(&f, id, &alice, 700, 1_000, 0x11);

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    assert!(f.client.get_seal(&id, &alice).is_some());

    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    assert!(
        f.client.try_reveal(&id, &alice, &700, &nonce).is_err(),
        "late reveal must be rejected after reveal_deadline"
    );
    assert_eq!(f.client.get_bid_state(&id, &alice).revealed_value, None);
}

#[test]
fn clear_and_settle_work_after_seal_expiry() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);
    let alice = funded_bidder(&f, 1_000);
    let bob = funded_bidder(&f, 1_000);
    let a_nonce = commit_bid(&f, id, &alice, 700, 1_000, 0x11);
    let b_nonce = commit_bid(&f, id, &bob, 500, 1_000, 0x22);

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    f.client.reveal(&id, &alice, &700, &a_nonce);
    f.client.reveal(&id, &bob, &500, &b_nonce);

    let ttl = temporary_seal_ttl(&f, &seal_key(id, &alice));
    advance_ledgers(&f, ttl + 1);
    assert!(f.client.get_seal(&id, &alice).is_none());
    assert!(f.client.get_seal(&id, &bob).is_none());

    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    assert_eq!(f.client.clear(&id), Some(alice.clone()));
    f.client.settle(&id);
    assert_eq!(f.client.get_round(&id).status, Status::Settled);
    assert_eq!(f.usdc_token.balance(&operator), 700);
}

#[test]
fn observer_reads_round_and_bid_state_after_lifecycle_completion() {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, ClearingRule::HighestBid);
    let alice = funded_bidder(&f, 1_000);
    let nonce = commit_bid(&f, id, &alice, 700, 1_000, 0x11);

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    f.client.reveal(&id, &alice, &700, &nonce);
    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    f.client.clear(&id);
    f.client.settle(&id);

    let ttl = temporary_seal_ttl(&f, &seal_key(id, &alice));
    advance_ledgers(&f, ttl + 1);

    assert!(f.client.get_seal(&id, &alice).is_none());
    let round = f.client.get_round(&id);
    assert_eq!(round.status, Status::Settled);
    assert_eq!(round.winner, Some(alice.clone()));
    assert_eq!(round.winning_bid, 700);
    let state = f.client.get_bid_state(&id, &alice);    assert_eq!(state.revealed_value, Some(700));
    assert!(state.valid);
    assert!(state.settled);
}

// ─────────────────────────────────────────────────────────────────────────────
// ISSUE #160 — ERROR CODE DOCUMENTATION CONSISTENCY
//
// These tests make sure contracts/round/ERRORS.md never drifts away from the
// exported enum Error in src/types.rs. Two complementary guards:
//
//   1. `variant_name` is a non-exhaustive-friendly match — adding, renaming, or
//      removing a variant in `src/types.rs` will fail to compile here. That is
//      the strongest guard.
//
//   2. `DOCUMENTED_ERROR_CODES` mirrors the same set of variants with their
//      runtime discriminants; the assertions below catch any silent reordering
//      or renumbering that does not change the variant list.
//
// Whenever you change the `Error` enum, update `contracts/round/ERRORS.md`,
// `DOCUMENTED_ERROR_CODES`, and `variant_name` in lock-step.
// ─────────────────────────────────────────────────────────────────────────────

/// Authoritative (name, code) mapping for every variant of [`Error`]. Keep in
/// sync with `contracts/round/ERRORS.md`.
pub(super) const DOCUMENTED_ERROR_CODES: &[(Error, u32)] = &[
    // ── 1–4: initialization & lookup ──
    (Error::NotInitialized, 1),
    (Error::AlreadyInitialized, 2),
    (Error::RoundNotFound, 3),
    (Error::BidNotFound, 4),
    // ── 10–22: lifecycle & timing ──
    (Error::CommitClosed, 10),
    (Error::CommitNotClosed, 11),
    (Error::CommitDeadlineAfterReveal, 12),
    (Error::RevealNotOpen, 13),
    (Error::RevealAlreadyOpen, 14),
    (Error::RevealWindowClosed, 15),
    (Error::RevealStillOpen, 16),
    (Error::NotCleared, 17),
    (Error::AlreadyCleared, 18),
    (Error::AlreadySettled, 19),
    (Error::RoundVoided, 20),
    (Error::NotVoidable, 21),
    (Error::WrongStatus, 22),
    // ── 30–39: cryptography & validation ──
    (Error::InvalidDrandSignature, 30),
    (Error::HashMismatch, 31),
    (Error::AlreadyRevealed, 32),
    (Error::PayloadTooLarge, 33),
    (Error::InvalidAmount, 34),
    (Error::BidExceedsEscrow, 35),
    (Error::DeadlineInPast, 36),
    (Error::NoValidBids, 37),
    (Error::RoundFull, 38),
    (Error::InvalidLimit, 39),
];

/// Convert an `Error` to its on-chain discriminant using the [`repr(u32)`]
/// representation declared in `src/types.rs`. This is the same value that is
/// embedded in `soroban_sdk::Error::Contract(...)` instances seen by callers.
pub(super) fn discriminant(e: Error) -> u32 {
    e as u32
}

pub(super) fn variant_name(e: Error) -> &'static str {
    match e {
        Error::NotInitialized => "NotInitialized",
        Error::AlreadyInitialized => "AlreadyInitialized",
        Error::RoundNotFound => "RoundNotFound",
        Error::BidNotFound => "BidNotFound",
        Error::CommitClosed => "CommitClosed",
        Error::CommitNotClosed => "CommitNotClosed",
        Error::CommitDeadlineAfterReveal => "CommitDeadlineAfterReveal",
        Error::RevealNotOpen => "RevealNotOpen",
        Error::RevealAlreadyOpen => "RevealAlreadyOpen",
        Error::RevealWindowClosed => "RevealWindowClosed",
        Error::RevealStillOpen => "RevealStillOpen",
        Error::NotCleared => "NotCleared",
        Error::AlreadyCleared => "AlreadyCleared",
        Error::AlreadySettled => "AlreadySettled",
        Error::RoundVoided => "RoundVoided",
        Error::NotVoidable => "NotVoidable",
        Error::WrongStatus => "WrongStatus",
        Error::InvalidDrandSignature => "InvalidDrandSignature",
        Error::HashMismatch => "HashMismatch",
        Error::AlreadyRevealed => "AlreadyRevealed",
        Error::PayloadTooLarge => "PayloadTooLarge",
        Error::InvalidAmount => "InvalidAmount",
        Error::BidExceedsEscrow => "BidExceedsEscrow",
        Error::DeadlineInPast => "DeadlineInPast",
        Error::NoValidBids => "NoValidBids",
        Error::RoundFull => "RoundFull",
        Error::InvalidLimit => "InvalidLimit",
    }
}

#[test]
fn error_discriminants_match_document() {
    for (variant, expected_code) in DOCUMENTED_ERROR_CODES {
        let actual = discriminant(*variant);
        assert_eq!(
            actual, *expected_code,
            "{} discriminant drifted (got {}, expected {}) — update \
             contracts/round/ERRORS.md and DOCUMENTED_ERROR_CODES together",
            variant_name(*variant),
            actual,
            expected_code,
        );
    }
}

#[test]
fn error_codes_have_no_duplicate_discriminants() {
    // O(n²) is fine: n = 27. Done without `std::collections` because the
    // contract's `#![no_std]` applies to this module.
    for (i, (variant_a, code_a)) in DOCUMENTED_ERROR_CODES.iter().enumerate() {
        let name_a = variant_name(*variant_a);
        for (variant_b, code_b) in DOCUMENTED_ERROR_CODES.iter().skip(i + 1) {
            if code_a == code_b {
                let name_b = variant_name(*variant_b);
                panic!(
                    "duplicate discriminant {code_a}: both {name_a} and {name_b} claim it. \
                     Two variants must not share an on-chain code."
                );
            }
        }
    }
}

#[test]
fn error_table_enumerates_every_variant() {
    // Maintenance hint: this test pins the *count* of variants in
    // DOCUMENTED_ERROR_CODES. The stronger parity guard is compile-time:
    // `variant_name` exhaustively matches every variant of `enum Error`, so
    // adding, removing, or renaming a variant fails to compile here. This
    // test just documents the expected scale and catches the sneakier case
    // where someone adds a variant AND a `variant_name` arm without updating
    // DOCUMENTED_ERROR_CODES.
    assert_eq!(
        DOCUMENTED_ERROR_CODES.len(),
        27,
        "DOCUMENTED_ERROR_CODES appears missing entries. The exhaustive \
         `variant_name` match already enforces parity at compile time — \
         update it together with this list and contracts/round/ERRORS.md."
    );
}

#[test]
fn error_codes_use_reserved_ranges() {
    // Range policy enforced by the documentation:
    //   1–4     → initialization/lookup
    //   10–22   → lifecycle/timing
    //   30–39   → crypto/validation
    // New categories should pick a fresh, contiguous range — not collide with
    // logging conventions — and update ERRORS.md at the same time.
    for (variant, code) in DOCUMENTED_ERROR_CODES {
        let name = variant_name(*variant);
        let in_range = matches!(*code, 1..=4 | 10..=22 | 30..=39);
        assert!(
            in_range,
            "{name} = {code} falls outside the documented code ranges; \
             update contracts/round/ERRORS.md if you intentionally added a new category"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ISSUE #385 — SHARED SETTLEMENT FIXTURE (contract half)
//
// `contracts/round/fixtures/settlement-cases.txt` is read by BOTH suites:
//
//   contracts/round/src/test.rs                    — this file: drives the
//       round contract through every row and asserts the exact payout or the
//       exact error the contract answers with.
//   services/keeper/src/settlement-guard.test.ts    — drives the keeper's
//       settlement guard through the same rows and asserts its typed refusal
//       (or its settlement plan).
//
// The rows the two suites used to disagree about are exactly the ones the
// contract rejects: the guard used to submit a settle or a void the contract
// reverts with `RoundVoided` / `NotVoidable`. Both halves now assert the same
// numbers from the same file, so the guard's rules cannot drift from the
// contract's without one of the two suites failing.
// ─────────────────────────────────────────────────────────────────────────────

const SETTLEMENT_FIXTURE: &str = include_str!("../fixtures/settlement-cases.txt");
const FIXTURE_MAX_BIDS: usize = 8;
const FIXTURE_MAX_REFUNDS: usize = 8;

/// Guard refusal → the contract error the very same view must fail with.
/// These are the rules both suites promise to implement identically.
const GUARD_REASON_CONTRACT_ERROR: &[(&str, &str)] = &[
    ("already_settled", "AlreadySettled"),
    ("round_voided", "RoundVoided"),
    ("not_cleared", "NotCleared"),
    ("missing_winner", "NoValidBids"),
    ("void_not_open", "NotVoidable"),
    ("void_grace_not_elapsed", "NotVoidable"),
];

/// Guard refusals that exist only because the keeper's *local view* is
/// incomplete (truncated bidder page, unreadable bid state, a stale winner).
/// The contract may well accept such a transaction — the guard still refuses,
/// because it cannot promise a refund set it never read.
const LOCAL_VIEW_GUARD_REASONS: &[&str] =
    &["refund_missing", "bidder_page_incomplete", "winner_mismatch"];

struct SettlementCase {
    name: &'static str,
    action: &'static str,
    rule: &'static str,
    status: &'static str,
    grace: bool,
    bids: [i128; FIXTURE_MAX_BIDS],
    bid_count: usize,
    escrows: [i128; FIXTURE_MAX_BIDS],
    revealed: [bool; FIXTURE_MAX_BIDS],
    read: usize,
    winner_idx: i32,
    operator: i128,
    surplus: i128,
    refunds: [(usize, i128); FIXTURE_MAX_REFUNDS],
    refund_count: usize,
    guard_reason: &'static str,
    contract: &'static str,
}

fn fixture_field<'a, I: Iterator<Item = &'a str>>(it: &mut I) -> &'a str {
    it.next()
        .unwrap_or_else(|| panic!("settlement fixture row is missing a field"))
}

/// `?(500)` = the keeper could not read this bid state; the contract still
/// holds 500. The contract side only ever needs the real amount.
fn fixture_escrow(token: &str) -> i128 {
    let inner = token.strip_prefix("?(").unwrap_or(token);
    let inner = inner.strip_suffix(')').unwrap_or(inner);
    inner.parse::<i128>().expect("fixture escrow must be an amount")
}

fn parse_settlement_case(line: &'static str) -> SettlementCase {
    let mut it = line.split('|');
    let name = fixture_field(&mut it);
    let action = fixture_field(&mut it);
    let rule = fixture_field(&mut it);
    let status = fixture_field(&mut it);
    let grace = fixture_field(&mut it) == "1";

    let mut bids = [0i128; FIXTURE_MAX_BIDS];
    let mut bid_count = 0usize;
    for token in fixture_field(&mut it).split(',') {
        assert!(bid_count < FIXTURE_MAX_BIDS, "{name}: too many bids");
        bids[bid_count] = token.parse::<i128>().expect("fixture bid must be an amount");
        bid_count += 1;
    }

    let mut escrows = [0i128; FIXTURE_MAX_BIDS];
    let mut escrow_count = 0usize;
    for token in fixture_field(&mut it).split(',') {
        assert!(escrow_count < FIXTURE_MAX_BIDS, "{name}: too many escrows");
        escrows[escrow_count] = fixture_escrow(token);
        escrow_count += 1;
    }
    assert_eq!(bid_count, escrow_count, "{name}: bids and escrows must align");

    let mut revealed = [false; FIXTURE_MAX_BIDS];
    let mut reveal_count = 0usize;
    for token in fixture_field(&mut it).split(',') {
        assert!(reveal_count < FIXTURE_MAX_BIDS, "{name}: too many revealed flags");
        revealed[reveal_count] = token == "1";
        reveal_count += 1;
    }
    assert_eq!(bid_count, reveal_count, "{name}: bids and revealed flags must align");

    let read = fixture_field(&mut it).parse::<usize>().expect("fixture read must be a count");
    let winner_idx = fixture_field(&mut it).parse::<i32>().expect("fixture winner_idx must be an index");
    let operator = fixture_field(&mut it).parse::<i128>().expect("fixture operator must be an amount");
    let surplus = fixture_field(&mut it).parse::<i128>().expect("fixture surplus must be an amount");

    let mut refunds = [(0usize, 0i128); FIXTURE_MAX_REFUNDS];
    let mut refund_count = 0usize;
    let refunds_field = fixture_field(&mut it);
    if refunds_field != "-" {
        for token in refunds_field.split(',') {
            assert!(refund_count < FIXTURE_MAX_REFUNDS, "{name}: too many refunds");
            let mut parts = token.split(':');
            let idx = fixture_field(&mut parts).parse::<usize>().expect("refund index");
            let amount = fixture_field(&mut parts).parse::<i128>().expect("refund amount");
            assert!(parts.next().is_none(), "{name}: malformed refund {token}");
            refunds[refund_count] = (idx, amount);
            refund_count += 1;
        }
    }

    let guard_reason = fixture_field(&mut it);
    let contract = fixture_field(&mut it);
    assert!(it.next().is_none(), "{name}: fixture row has trailing fields");

    assert!(read <= bid_count, "{name}: keeper read exceeds the bidder index");
    assert!(
        matches!(action, "settle" | "void"),
        "{name}: action must be settle or void"
    );

    SettlementCase {
        name,
        action,
        rule,
        status,
        grace,
        bids,
        bid_count,
        escrows,
        revealed,
        read,
        winner_idx,
        operator,
        surplus,
        refunds,
        refund_count,
        guard_reason,
        contract,
    }
}

fn contract_error_from_name(name: &str, case: &str) -> Error {
    for (variant, _) in DOCUMENTED_ERROR_CODES {
        if variant_name(*variant) == name {
            return *variant;
        }
    }
    panic!("{case}: {name} is not a contract error in src/types.rs")
}

fn status_from_name(name: &str) -> Status {
    match name {
        "Open" => Status::Open,
        "Revealing" => Status::Revealing,
        "Cleared" => Status::Cleared,
        "Settled" => Status::Settled,
        "Voided" => Status::Voided,
        other => panic!("unknown round status {other} in the settlement fixture"),
    }
}

struct CaseRound {
    f: Fixture,
    id: u64,
    bidders: Vec<Address>,
    operator: Address,
    reveal_deadline: u64,
}

/// Build the on-chain round a fixture row describes: same bids, same escrow,
/// the same reveal pattern, and the same status when the action is asked for.
fn build_case_round(c: &SettlementCase) -> CaseRound {
    let (f, t_reveal, commit_deadline, reveal_deadline) = setup_drand();
    let operator = Address::generate(&f.env);
    let rule = match c.rule {
        "LowestBid" => ClearingRule::LowestBid,
        "HighestBid" => ClearingRule::HighestBid,
        other => panic!("{}: unknown clearing rule {other}", c.name),
    };
    let id = drand_round(&f, &operator, commit_deadline, reveal_deadline, rule);

    let mut bidders = Vec::new(&f.env);
    let mut nonces: Vec<BytesN<32>> = Vec::new(&f.env);
    for i in 0..c.bid_count {
        let bidder = funded_bidder(&f, c.escrows[i]);
        let nonce = commit_bid(&f, id, &bidder, c.bids[i], c.escrows[i], (i as u8) + 1);
        bidders.push_back(bidder);
        nonces.push_back(nonce);
    }

    // An Open round never opens the reveal window: `void` must be judged on
    // status + grace alone.
    if c.status == "Open" {
        return CaseRound { f, id, bidders, operator, reveal_deadline };
    }

    f.env.ledger().with_mut(|l| l.timestamp = t_reveal + 1);
    f.client.open_reveal(&id, &real_sig(&f.env));
    for i in 0..c.bid_count {
        if c.revealed[i] {
            f.client.reveal(
                &id,
                &bidders.get(i as u32).unwrap(),
                &c.bids[i],
                &nonces.get(i as u32).unwrap(),
            );
        }
    }

    // A fully revealed round that has not been cleared yet: `void` must be
    // judged on the status rule alone.
    if c.status == "Revealing" {
        return CaseRound { f, id, bidders, operator, reveal_deadline };
    }

    f.env.ledger().with_mut(|l| l.timestamp = reveal_deadline + 1);
    let winner = f.client.clear(&id);
    match winner {
        Some(w) => {
            assert!(
                c.winner_idx >= 0,
                "{}: clear found a winner but the fixture says -1",
                c.name
            );
            assert_eq!(
                w,
                bidders.get(c.winner_idx as u32).unwrap(),
                "{}: winner index",
                c.name
            );
        }
        None => assert_eq!(
            c.winner_idx,
            -1,
            "{}: clear found no winner but the fixture says index {}",
            c.name,
            c.winner_idx
        ),
    }

    CaseRound { f, id, bidders, operator, reveal_deadline }
}

fn run_settlement_case(c: &SettlementCase) {
    let case = build_case_round(c);
    let f = &case.f;
    assert_eq!(
        f.client.get_round(&case.id).status,
        status_from_name(c.status),
        "{}: status before the action",
        c.name
    );
    // `read` is how much of the index the keeper's local view saw; the
    // contract always reads the whole index itself, so all that matters here
    // is that the row is well-formed. The truncation itself is the guard's
    // half of the fixture (`bidder_page_incomplete`).
    assert!(
        c.read <= c.bid_count,
        "{}: keeper read {} of {} bidders",
        c.name,
        c.read,
        c.bid_count
    );

    // `grace = 1` places the clock past reveal_deadline + VOID_GRACE, `0`
    // inside the window — the exact boundary `SubRosaRound::void` enforces.
    let now = if c.grace {
        case.reveal_deadline + 3_601
    } else {
        case.reveal_deadline + 100
    };
    f.env.ledger().with_mut(|l| l.timestamp = now);

    if c.contract == "ok" {
        if c.action == "settle" {
            f.client.settle(&case.id);
            assert_eq!(
                f.client.get_round(&case.id).status,
                Status::Settled,
                "{}: settle must land",
                c.name
            );
        } else {
            f.client.void(&case.id);
            assert_eq!(
                f.client.get_round(&case.id).status,
                Status::Voided,
                "{}: void must land",
                c.name
            );
        }

        assert_eq!(
            f.usdc_token.balance(&case.operator),
            c.operator,
            "{}: operator payout",
            c.name
        );
        if c.winner_idx >= 0 {
            assert_eq!(
                f.usdc_token.balance(&case.bidders.get(c.winner_idx as u32).unwrap()),
                c.surplus,
                "{}: winner surplus",
                c.name
            );
        } else {
            assert_eq!(c.surplus, 0, "{}: surplus without a winner", c.name);
        }
        for i in 0..c.refund_count {
            let (idx, amount) = c.refunds[i];
            assert_eq!(
                f.usdc_token.balance(&case.bidders.get(idx as u32).unwrap()),
                amount,
                "{}: refund for bidder {idx}",
                c.name
            );
        }
        assert_eq!(
            f.usdc_token.balance(&f.client.address),
            0,
            "{}: contract must be drained",
            c.name
        );
    } else {
        let expected = contract_error_from_name(c.contract, c.name);
        if c.action == "settle" {
            assert_try_contract_err(f.client.try_settle(&case.id), expected);
        } else {
            assert_try_contract_err(f.client.try_void(&case.id), expected);
        }
        assert_eq!(
            f.client.get_round(&case.id).status,
            status_from_name(c.status),
            "{}: a rejected action must not move the round",
            c.name
        );
    }
}

/// Iterate every data row of the shared fixture (comments and the header
/// line are skipped, exactly as the keeper suite does).
fn for_each_settlement_case(mut run: impl FnMut(&SettlementCase)) {
    let mut count = 0usize;
    for line in SETTLEMENT_FIXTURE.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("name|") {
            continue;
        }
        let case = parse_settlement_case(line);
        run(&case);
        count += 1;
    }
    assert!(
        count >= 10,
        "the shared settlement fixture must keep every row (parsed {count})"
    );
}

#[test]
fn settlement_fixture_drives_the_contract() {
    for_each_settlement_case(run_settlement_case);
}

/// The agreement rule both suites assert: a row the contract rejects always
/// carries the guard refusal that maps back to that exact error, a row the
/// guard submits is always a row the contract accepts, and a local-view
/// refusal never claims the contract would reject on its own.
#[test]
fn settlement_fixture_guard_reasons_match_contract_rules() {
    for_each_settlement_case(|c| {
        let mapped = GUARD_REASON_CONTRACT_ERROR
            .iter()
            .find(|(reason, _)| *reason == c.guard_reason)
            .map(|(_, error)| *error);
        let local_view = LOCAL_VIEW_GUARD_REASONS.contains(&c.guard_reason);

        if c.guard_reason == "-" {
            assert_eq!(
                c.contract, "ok",
                "{}: the guard submits only what the contract accepts",
                c.name
            );
        } else {
            assert!(
                mapped.is_some() || local_view,
                "{}: {} is not a guard reason both suites know",
                c.name,
                c.guard_reason
            );
            if let Some(error) = mapped {
                assert_eq!(
                    c.contract, error,
                    "{}: guard reason {} must be exactly why the contract rejects",
                    c.name, c.guard_reason
                );
            } else {
                assert_eq!(
                    c.contract, "ok",
                    "{}: a local-view refusal must never claim the contract rejects",
                    c.name
                );
            }
        }

        if c.contract != "ok" {
            assert_ne!(
                c.guard_reason, "-",
                "{}: the contract rejects with {} — the guard must refuse too",
                c.name,
                c.contract
            );
            // The error name itself has to be a real variant, not a typo.
            contract_error_from_name(c.contract, c.name);
        }
    });
}
