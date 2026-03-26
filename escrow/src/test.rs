use super::{LiquifactEscrow, LiquifactEscrowClient, SCHEMA_VERSION};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

// ── helpers ───────────────────────────────────────────────────────────────────

fn deploy(env: &Env) -> LiquifactEscrowClient<'_> {
    let id = env.register(LiquifactEscrow, ());
    LiquifactEscrowClient::new(env, &id)
}

fn default_init(client: &LiquifactEscrowClient, admin: &Address, sme: &Address) {
    client.init(
        admin,
        &symbol_short!("INV001"),
        sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Existing tests (compatible with current contract API)
// ══════════════════════════════════════════════════════════════════════════════

/// After `init` the escrow version must match the compiled schema constant.
#[test]
fn test_init_sets_version() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    let escrow = client.init(
        &admin,
        &symbol_short!("INV001"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    assert_eq!(escrow.version, SCHEMA_VERSION);
    assert_eq!(client.get_version(), SCHEMA_VERSION);
}

/// Re-initialization of the same contract must be rejected.
#[test]
#[should_panic(expected = "Escrow already initialized")]
fn test_reinit_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    default_init(&client, &admin, &sme);
    default_init(&client, &admin, &sme); // must panic
}

/// Funding after the escrow is already funded (status 1) must be rejected.
#[test]
#[should_panic(expected = "Escrow not open for funding")]
fn test_fund_after_funded_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV010"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &10_000_0000000i128); // status -> 1
    client.fund(&investor, &1i128); // must panic
}

/// get_escrow on an uninitialized contract must panic.
#[test]
#[should_panic(expected = "Escrow not initialized")]
fn test_get_escrow_uninitialized_panics() {
    let env = Env::default();
    let client = deploy(&env);
    client.get_escrow();
}

/// Migrating from the current version must be rejected.
#[test]
#[should_panic(expected = "Already at current schema version")]
fn test_migrate_at_current_version_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    default_init(&client, &admin, &sme);
    client.migrate(&SCHEMA_VERSION);
}

/// Migrating with a non-matching from_version must be rejected.
#[test]
#[should_panic(expected = "from_version does not match stored version")]
fn test_migrate_wrong_from_version_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    default_init(&client, &admin, &sme);
    client.migrate(&99u32);
}

/// Funding before init must panic with "Escrow not initialized".
#[test]
#[should_panic(expected = "Escrow not initialized")]
fn test_fund_fails_when_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let investor = Address::generate(&env);
    client.fund(&investor, &1000);
}

/// Partial funding keeps status 0; reaching target flips to 1.
#[test]
fn test_partial_then_full_funding() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV_PF"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    let partial = client.fund(&investor, &5_000_0000000i128);
    assert_eq!(partial.status, 0);
    assert_eq!(partial.funded_amount, 5_000_0000000i128);

    let full = client.fund(&investor, &5_000_0000000i128);
    assert_eq!(full.status, 1);
    assert_eq!(full.funded_amount, 10_000_0000000i128);
}

/// Settle before funding must be rejected.
#[test]
#[should_panic(expected = "Escrow must be funded before settlement")]
fn test_settle_before_funded_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    default_init(&client, &admin, &sme);
    client.settle(&1000i128); // must panic — status is still 0
}

/// Partial settlement flow: multiple settle calls accumulate until total_due.
#[test]
fn test_partial_settlement_flow() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV_PS"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    client.fund(&investor, &10_000_0000000i128);

    let interest = (10_000_0000000i128 * 800) / 10000;
    let total_due = 10_000_0000000i128 + interest; // 10,800,000,000

    // First partial: 5,000,000,000
    let e1 = client.settle(&5_000_0000000i128);
    assert_eq!(e1.settled_amount, 5_000_0000000i128);
    assert_eq!(e1.status, 1);

    // Second partial: 5,000,000,000
    let e2 = client.settle(&5_000_0000000i128);
    assert_eq!(e2.settled_amount, 10_000_0000000i128);
    assert_eq!(e2.status, 1);

    // Final settlement: 800,000,000 (the interest portion)
    let e3 = client.settle(&800_0000000i128);
    assert_eq!(e3.settled_amount, total_due);
    assert_eq!(e3.status, 2);
}

/// Over-settlement must be rejected.
#[test]
#[should_panic(expected = "Settlement amount exceeds total due")]
fn test_over_settlement_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV_OS"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &10_000_0000000i128);

    let interest = (10_000_0000000i128 * 800) / 10000;
    let total_due = 10_000_0000000i128 + interest;

    // Try to settle more than total_due in one shot
    client.settle(&(total_due + 1));
}

/// Update maturity in Open state succeeds.
#[test]
fn test_update_maturity_in_open_state() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    default_init(&client, &admin, &sme);
    let escrow = client.update_maturity(&2000u64);
    assert_eq!(escrow.maturity, 2000);
    assert_eq!(escrow.status, 0);
}

/// Update maturity after funding must be rejected.
#[test]
#[should_panic(expected = "Maturity can only be updated in Open state")]
fn test_update_maturity_after_funded_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    default_init(&client, &admin, &sme);
    client.fund(&investor, &10_000_0000000i128);
    client.update_maturity(&2000u64); // must panic — status is 1
}

// ══════════════════════════════════════════════════════════════════════════════
// Integer overflow / underflow validation tests
// (Issue: Validate Integer Overflow and Underflow Paths)
//
// These tests verify that all arithmetic operations in the contract use
// checked_add / checked_mul and produce explicit panics instead of silently
// wrapping on overflow.
// ══════════════════════════════════════════════════════════════════════════════

// ---------------------------------------------------------------------------
// Overflow panic tests (#[should_panic])
// ---------------------------------------------------------------------------

/// fund() must panic when funded_amount + amount overflows i128.
///
/// Setup: funding_target = i128::MAX so status stays 0 after the first fund.
/// First fund brings funded_amount to i128::MAX - 100.
/// Second fund of 200 causes checked_add to overflow.
#[test]
#[should_panic(expected = "Arithmetic overflow")]
fn test_fund_overflow_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("OVF_F1"),
        &sme,
        &i128::MAX, // funding_target = i128::MAX so status stays 0
        &0i64,
        &1000u64,
    );

    // Bring funded_amount to i128::MAX - 100 (still < funding_target, status = 0)
    client.fund(&investor, &(i128::MAX - 100));

    // This must overflow: (i128::MAX - 100) + 200 > i128::MAX
    client.fund(&investor, &200i128);
}

/// settle() must panic when interest = amount * yield_bps overflows i128.
///
/// With amount = i128::MAX / 2 and yield_bps = 30_000, the multiplication
/// produces a value far exceeding i128::MAX.
#[test]
#[should_panic(expected = "Arithmetic overflow")]
fn test_settle_interest_multiplication_overflow_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    let large_amount = i128::MAX / 2;

    client.init(
        &admin,
        &symbol_short!("OVF_S1"),
        &sme,
        &large_amount,
        &30_000i64, // 300% — guarantees multiplication overflow
        &1000u64,
    );

    // Fund exactly the target to reach status = 1
    client.fund(&investor, &large_amount);

    // settle triggers: amount.checked_mul(yield_bps as i128) → overflow
    client.settle(&1i128);
}

/// settle() must panic when total_due = amount + interest overflows i128.
///
/// With amount = i128::MAX and yield_bps = 1, the multiplication
/// i128::MAX * 1 fits in i128, but amount + interest overflows.
#[test]
#[should_panic(expected = "Arithmetic overflow")]
fn test_settle_total_due_addition_overflow_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("OVF_S2"),
        &sme,
        &i128::MAX,
        &1i64, // yield_bps = 1 → interest = i128::MAX / 10000
        &1000u64,
    );

    // Fund to reach status 1
    client.fund(&investor, &i128::MAX);

    // settle: interest = i128::MAX * 1 / 10000 ≈ 1.7e34
    //         total_due = i128::MAX + 1.7e34 → checked_add overflow
    client.settle(&1i128);
}

/// settle() must panic when settled_amount accumulation overflows i128.
///
/// First settle sets settled_amount = 1. Second settle with i128::MAX
/// triggers checked_add(1, i128::MAX) → overflow.
#[test]
#[should_panic(expected = "Arithmetic overflow")]
fn test_settle_settled_amount_overflow_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("OVF_S3"),
        &sme,
        &1_000i128,
        &800i64, // 8% → total_due = 1080
        &1000u64,
    );

    client.fund(&investor, &1_000i128);

    // First settle: settled_amount = 1
    client.settle(&1i128);

    // Second settle with i128::MAX: checked_add(1, i128::MAX) → overflow
    client.settle(&i128::MAX);
}

// ---------------------------------------------------------------------------
// Normal-case boundary value tests
// ---------------------------------------------------------------------------

/// Large but representable amount (10^18 stroops) must fund correctly.
#[test]
fn test_fund_large_but_valid_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    let amount = 1_000_000_000_000_000_000i128; // 10^18

    client.init(
        &admin,
        &symbol_short!("BND_F1"),
        &sme,
        &amount,
        &800i64,
        &1000u64,
    );

    let escrow = client.fund(&investor, &amount);
    assert_eq!(escrow.funded_amount, amount);
    assert_eq!(escrow.status, 1);
}

/// Large but representable interest calculation must succeed.
///
/// amount = 10^18, yield_bps = 800 (8%)
/// interest = 10^18 * 800 / 10000 = 8 * 10^16
/// total_due = 10^18 + 8*10^16 = 1.08 * 10^18
#[test]
fn test_settle_large_but_valid_interest() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    let amount = 1_000_000_000_000_000_000i128; // 10^18

    client.init(
        &admin,
        &symbol_short!("BND_S1"),
        &sme,
        &amount,
        &800i64,
        &1000u64,
    );

    client.fund(&investor, &amount);

    let interest = amount * 800 / 10000; // 8 * 10^16
    let total_due = amount + interest;    // 1.08 * 10^18

    let escrow = client.settle(&total_due);
    assert_eq!(escrow.settled_amount, total_due);
    assert_eq!(escrow.status, 2);
}

// ---------------------------------------------------------------------------
// Property-based test (proptest)
// ---------------------------------------------------------------------------

use proptest::prelude::*;

proptest! {
    /// For any pair of safe i128 values, fund() must produce exactly the
    /// arithmetic sum (no silent wrapping). Values are capped at 10^18 so
    /// their sum never exceeds i128::MAX, isolating the invariant from
    /// overflow-path concerns (which are covered by the should_panic tests).
    #[test]
    fn prop_fund_never_silently_overflows(
        initial in 1i128..1_000_000_000_000_000_000i128,
        funding in 1i128..1_000_000_000_000_000_000i128,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let sme = Address::generate(&env);
        let investor = Address::generate(&env);
        let client = deploy(&env);

        // funding_target = i128::MAX keeps status at 0 across multiple funds
        client.init(
            &admin,
            &symbol_short!("PROPTST"),
            &sme,
            &i128::MAX,
            &0i64,
            &1000u64,
        );

        // First fund sets funded_amount = initial
        client.fund(&investor, &initial);

        // Second fund: result must equal initial + funding exactly
        let escrow = client.fund(&investor, &funding);
        prop_assert_eq!(escrow.funded_amount, initial + funding);
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Legacy tests preserved from feature branches (not compiled)
//
// These tests reference APIs from feature branches not yet merged into the
// current contract (EscrowFactory, pause/unpause, confirm_payment, buyer
// address, metadata hash, etc.).  They are preserved verbatim below so that
// they can be re-enabled when those features land.
// ══════════════════════════════════════════════════════════════════════════════

/*  ── BEGIN LEGACY ──────────────────────────────────────────────────────────

use super::{LiquifactEscrow, LiquifactEscrowClient, SCHEMA_VERSION};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};
//

// ── helpers ───────────────────────────────────────────────────────────────────

fn deploy(env: &Env) -> LiquifactEscrowClient<'_> {
    let id = env.register(LiquifactEscrow, ());
    LiquifactEscrowClient::new(env, &id)
}

fn default_init(client: &LiquifactEscrowClient, admin: &Address, sme: &Address) {
    client.init(
        admin,
        &symbol_short!("INV001"),
        sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
}

// ── init ──────────────────────────────────────────────────────────────────────

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

fn deploy(env: &Env) -> (Address, LiquifactEscrowClient<'_>) {
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(env, &contract_id);
    (contract_id, client)
}

// ──────────────────────────────────────────────────────────────────────────────
// init
// ──────────────────────────────────────────────────────────────────────────────

/// After `init` the escrow must be open (status 0) with zero funded_amount,
/// and `get_escrow` must return an identical snapshot.
#[test]
fn test_init_sets_version() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    let escrow = client.init(
        &admin,
        &symbol_short!("INV001"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
}

/// Two separate ContractState instances are independent.
#[test]
fn test_init_with_admin_independent_states() {
    let state_a = EscrowContract::init_with_admin(Address::from_string("GADMIN_A"));
    let state_b = EscrowContract::init_with_admin(Address::from_string("GADMIN_B"));
    assert_ne!(state_a.admin, state_b.admin);
    assert!(!state_a.paused);
    assert!(!state_b.paused);
}

// ===========================================================================
// pause() tests  (Issue #24)
// ===========================================================================

/// Admin can pause an unpaused contract.
#[test]
fn test_pause_by_admin_succeeds() {
    let mut state = unpaused_state();
    EscrowContract::pause(&mut state, &make_admin());
    assert!(state.paused, "contract must be paused after pause()");
}

/// is_paused() returns true immediately after pause.
#[test]
fn test_pause_is_reflected_in_is_paused() {
    let mut state = unpaused_state();
    EscrowContract::pause(&mut state, &make_admin());
    assert!(EscrowContract::is_paused(&state));
}

/// Non-admin caller must be rejected.
#[test]
#[should_panic(expected = "caller is not admin")]
fn test_pause_by_non_admin_panics() {
    let mut state = unpaused_state();
    EscrowContract::pause(&mut state, &make_other());
}

/// Pausing an already-paused contract must be rejected.
#[test]
#[should_panic(expected = "contract already paused")]
fn test_pause_when_already_paused_panics() {
    let mut state = paused_state();
    EscrowContract::pause(&mut state, &make_admin());
}

/// Non-admin on already-paused: non-admin check fires first.
#[test]
#[should_panic(expected = "caller is not admin")]
fn test_pause_non_admin_on_paused_contract_panics() {
    let mut state = paused_state();
    EscrowContract::pause(&mut state, &make_other());
}

// ===========================================================================
// unpause() tests  (Issue #24)
// ===========================================================================

/// Admin can unpause a paused contract.
#[test]
fn test_unpause_by_admin_succeeds() {
    let mut state = paused_state();
    EscrowContract::unpause(&mut state, &make_admin());
    assert!(!state.paused, "contract must be unpaused after unpause()");
}

/// is_paused() returns false after unpause.
#[test]
fn test_unpause_is_reflected_in_is_paused() {
    let mut state = paused_state();
    EscrowContract::unpause(&mut state, &make_admin());
    assert!(!EscrowContract::is_paused(&state));
}

/// Non-admin caller must be rejected.
#[test]
#[should_panic(expected = "caller is not admin")]
fn test_unpause_by_non_admin_panics() {
    let mut state = paused_state();
    EscrowContract::unpause(&mut state, &make_other());
}

/// Unpausing an already-unpaused contract must be rejected.
#[test]
#[should_panic(expected = "contract not paused")]
fn test_unpause_when_not_paused_panics() {
    let mut state = unpaused_state();
    EscrowContract::unpause(&mut state, &make_admin());
}

// ===========================================================================
// is_paused() tests  (Issue #24)
// ===========================================================================

#[test]
fn test_is_paused_false_initially() {
    let state = unpaused_state();
    assert!(!EscrowContract::is_paused(&state));
}

#[test]
fn test_is_paused_true_after_pause() {
    let state = paused_state();
    assert!(EscrowContract::is_paused(&state));
}

#[test]
fn test_is_paused_false_after_unpause() {
    let mut state = paused_state();
    EscrowContract::unpause(&mut state, &make_admin());
    assert!(!EscrowContract::is_paused(&state));
}

#[test]
fn test_is_paused_does_not_mutate() {
    let state = paused_state();
    let _ = EscrowContract::is_paused(&state);
    let _ = EscrowContract::is_paused(&state);
    assert!(state.paused);
}

// ===========================================================================
// fund() — pause guard tests  (Issue #24)
// ===========================================================================

#[test]
#[should_panic(expected = "contract is paused")]
fn test_fund_blocked_when_paused() {
    let state = paused_state();
    let mut escrow = default_escrow();
    EscrowContract::fund(&state, &mut escrow, 100_000);
}

#[test]
fn test_fund_allowed_when_unpaused() {
    let state = unpaused_state();
    let mut escrow = default_escrow();
    EscrowContract::fund(&state, &mut escrow, 500_000);
    assert_eq!(escrow.funded_amount, 500_000);
}

    assert_eq!(escrow.version, SCHEMA_VERSION);
    assert_eq!(client.get_version(), SCHEMA_VERSION);
}

#[test]
fn test_init_and_get_escrow() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    let escrow = client.init(
        &admin,
        &symbol_short!("INV001"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
        &test_hash(&env),
    );
    assert_eq!(escrow.invoice_id, symbol_short!("INV001"));
    assert_eq!(escrow.admin, admin);
    assert_eq!(escrow.sme_address, sme);
    assert_eq!(escrow.buyer_address, buyer);
    assert_eq!(escrow.amount, 10_000_0000000i128);
    assert_eq!(escrow.funding_target, 10_000_0000000i128);
    assert_eq!(escrow.funded_amount, 0);
    assert_eq!(escrow.yield_bps, 800);
    assert_eq!(escrow.maturity, 1000);
    assert_eq!(escrow.status, 0);
}

#[test]
#[should_panic(expected = "Escrow amount must be positive")]
fn test_init_with_zero_fails() {
    let env = Env::default();
    let (client, sme, id) = setup_test(&env);
    client.init(&id, &sme, &0, &800, &10000);
}

    // get_escrow must match what init returned
    let got = client.get_escrow();
    assert_eq!(got.invoice_id, escrow.invoice_id);
    assert_eq!(got.admin, admin);
    assert_eq!(got.metadata_hash, test_hash(&env));
}

/// `init` must emit exactly one `EscrowInitialized` event whose payload
/// matches the returned snapshot.
///
/// `env.events().all()` captures events from the last invocation only — this
/// works perfectly since init is the only call in this test.
#[test]
#[should_panic(expected = "Escrow already initialized")]
fn test_reinit_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    default_init(&client, &admin, &sme);
    default_init(&client, &admin, &sme); // must panic
}

// ── fund & settle ─────────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Funding amount must be positive")]
fn test_fund_with_zero_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let admin = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV002"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
        &test_hash(&env),
    );
    client.fund(&investor, &1_000i128); // reaches funded status
    client.fund(&investor, &1i128); // must panic
}

    let e1 = client.fund(&investor, &10_000_0000000i128);
    assert_eq!(e1.funded_amount, 10_000_0000000i128);
    assert_eq!(e1.status, 1);

    let e2 = client.settle();
    assert_eq!(e2.status, 2);
}

#[test]
fn test_partial_fund_stays_open() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV003"),
        &sme,
        &buyer,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
        &test_hash(&env),
    );

    let partial = client.fund(&investor, &5_000_0000000i128);
    assert_eq!(partial.status, 0);
    assert_eq!(partial.funded_amount, 5_000_0000000i128);

    let full = client.fund(&investor, &5_000_0000000i128);
    assert_eq!(full.status, 1);
    assert_eq!(full.funded_amount, 10_000_0000000i128);
}

#[test]
#[should_panic(expected = "Escrow not open for funding")]
fn test_fund_after_funded_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV010"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &10_000_0000000i128); // status -> 1
    client.fund(&investor, &1i128); // must panic
}

#[test]
#[should_panic(expected = "Escrow must be funded before settlement")]
fn test_settle_before_funded_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    default_init(&client, &admin, &sme);
    client.settle(); // must panic — status is still 0
}

// ── auth checks ───────────────────────────────────────────────────────────────

#[test]
fn test_fund_records_investor_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let buyer = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV010"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
        &test_hash(&env),
    );
    client.fund(&investor, &1_000i128);

    assert!(
        env.auths().iter().any(|(addr, _)| *addr == investor),
        "investor auth was not recorded for fund"
    );
}

#[test]
fn test_settle_records_sme_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let buyer = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV005"),
        &sme,
        &buyer,
        &1_000i128,
        &500i64,
        &2000u64,
        &test_hash(&env),
    );
    client.fund(&investor, &1_000i128);
    client.confirm_payment();
    client.settle();

    assert!(
        env.auths().iter().any(|(addr, _)| *addr == sme),
        "sme auth was not recorded for settle"
    );
}

// ── get_escrow uninitialized ──────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Escrow not initialized")]
fn test_get_escrow_uninitialized_panics() {
    let env = Env::default();
    let client = deploy(&env);
    client.get_escrow();
}

// ── migration guards ──────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Already at current schema version")]
fn test_migrate_at_current_version_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    default_init(&client, &admin, &sme);
    client.migrate(&SCHEMA_VERSION);
}

#[test]
#[should_panic(expected = "from_version does not match stored version")]
fn test_migrate_wrong_from_version_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    default_init(&client, &admin, &sme);
    client.migrate(&99u32);
}

use proptest::prelude::*;

proptest! {
    // Escrow Property Invariants

    #[test]
    fn prop_funded_amount_non_decreasing(
        amount1 in 0..10_000_0000000i128,
        amount2 in 0..10_000_0000000i128
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let sme = Address::generate(&env);
        let investor1 = Address::generate(&env);
        let investor2 = Address::generate(&env);

        let contract_id = env.register(LiquifactEscrow, ());
        let client = LiquifactEscrowClient::new(&env, &contract_id);

        let target_amount = 20_000_0000000i128;

        client.init(
            &symbol_short!("INVTST"),
            &sme,
            &target_amount,
            &800i64,
            &1000u64,
        );

        // First funding
        let pre_funding_amount = client.get_escrow().funded_amount;
        client.fund(&investor1, &amount1);
        let post_funding1 = client.get_escrow().funded_amount;

        // Invariant: Funding amount acts monotonically
        assert!(post_funding1 >= pre_funding_amount, "Funded amount should be non-decreasing");

        // Skip second funding if status already flipped
        if client.get_escrow().status == 0 {
            client.fund(&investor2, &amount2);
            let post_funding2 = client.get_escrow().funded_amount;
            assert!(post_funding2 >= post_funding1, "Funded amount should be non-decreasing on successive funds");
        }
    }

    #[test]
    fn prop_bounded_status_transitions(
        amount in 0..50_000_0000000i128,
        target_amount in 100..10000_000000i128,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let sme = Address::generate(&env);
        let investor = Address::generate(&env);

        let contract_id = env.register(LiquifactEscrow, ());
        let client = LiquifactEscrowClient::new(&env, &contract_id);

        let escrow = client.init(
            &symbol_short!("INVSTA"),
            &sme,
            &target_amount,
            &800i64,
            &1000u64,
        );

        // Initial status is 0
        assert_eq!(escrow.status, 0);

        // Status bounds check
        assert!(escrow.status <= 2);

        let funded_escrow = client.fund(&investor, &amount);

        // Mid-status bounds check
        assert!(funded_escrow.status <= 2);

        // Ensure status 1 is reached ONLY if target met
        if amount >= target_amount {
            assert_eq!(funded_escrow.status, 1);

            // Only funded escrows can be settled
            let settled_escrow = client.settle();
            assert_eq!(settled_escrow.status, 2);
        } else {
            // Unfunded remains 0
            assert_eq!(funded_escrow.status, 0);
        }
    }
}

#[test]
#[should_panic(expected = "Escrow not initialized")]
fn test_fund_fails_when_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let investor = Address::generate(&env);
    client.fund(&investor, &1000);
}

#[test]
#[should_panic(expected = "Escrow not initialized")]
fn test_settle_fails_when_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.settle();
}

#[test]
#[should_panic(expected = "Escrow must be funded before settlement")]
fn test_settle_fails_when_not_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let sme = Address::generate(&env);
    client.init(&symbol_short!("INV001"), &sme, &1000, &800, &1000);
    client.settle();
}

#[test]
#[should_panic(expected = "Escrow not open for funding")]
fn test_fund_fails_when_already_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(&symbol_short!("INV001"), &sme, &1000, &800, &1000);
    client.fund(&investor, &1000);
    // Escrow is now funded status = 1.
    client.fund(&investor, &500); // Should panic
}

#[test]
#[should_panic(expected = "Escrow must be funded before settlement")]
fn test_settle_fails_when_already_settled() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(&symbol_short!("INV001"), &sme, &1000, &800, &1000);
    client.fund(&investor, &1000);
    client.settle();

    // Already settled status = 2, status != 1 so expect panic
    client.settle();
}

#[test]
fn test_fund_does_not_enforce_investor_auth() {
    let env = Env::default();
    // SECURITY: We do not call env.mock_all_auths() here to prove that
    // the contract does *not* enforce require_auth() on the investor.
    // If it did, this test would fail because there are no mocked auths.

    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(&symbol_short!("INV001"), &sme, &1000, &800, &1000);
    let escrow = client.fund(&investor, &1000);

    assert_eq!(escrow.funded_amount, 1000);
    assert_eq!(escrow.status, 1);
}

#[test]
fn test_settle_does_not_enforce_auth() {
    let env = Env::default();
    // SECURITY: Proves settle can be called by anyone without require_auth().

    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(&symbol_short!("INV001"), &sme, &1000, &800, &1000);
    client.fund(&investor, &1000);
    let escrow = client.settle();

    assert_eq!(escrow.status, 2);
}

#[test]
fn test_reinit_overwrites_escrow() {
    let env = Env::default();
    // SECURITY: Show that init can be called again by anyone to overwrite the escrow.
    env.mock_all_auths();

    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let sme1 = Address::generate(&env);
    let sme2 = Address::generate(&env);

    client.init(&symbol_short!("INV001"), &sme1, &1000, &800, &1000);
    let escrow1 = client.get_escrow();
    assert_eq!(escrow1.sme_address, sme1);

    // Someone else overwrites it
    client.init(&symbol_short!("ATTACK"), &sme2, &9999, &999, &9999);
    let escrow2 = client.get_escrow();
    assert_eq!(escrow2.sme_address, sme2);
    assert_eq!(escrow2.invoice_id, symbol_short!("ATTACK"));
}

#[test]
fn test_partial_settlement_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let expected_event = EscrowInitialized {
        name: symbol_short!("escrow_ii"),
        escrow: escrow.clone(),
    };

    assert_eq!(
        env.events().all(),
        std::vec![expected_event.to_xdr(&env, &contract_id)],
        "EscrowInitialized event must match the returned InvoiceEscrow snapshot"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// fund
// ──────────────────────────────────────────────────────────────────────────────

/// Partial funding keeps status at 0; full funding flips status to 1.
#[test]
fn test_partial_then_full_fund() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, client) = deploy(&env);
    let sme = Address::generate(&env);
    let admin = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(
        &symbol_short!("INV_P1"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    client.fund(&investor, &10_000_0000000i128);

    let interest = (10_000_0000000i128 * 800) / 10000;
    let total_due = 10_000_0000000i128 + interest; // 10,800,000,000

    // First partial: 5,000,000,000
    let e1 = client.settle(&5_000_0000000i128);
    assert_eq!(e1.settled_amount, 5_000_0000000i128);
    assert_eq!(e1.status, 1);

    // Second partial: 5,000,000,000
    let e2 = client.settle(&5_000_0000000i128);
    assert_eq!(e2.settled_amount, 10_000_0000000i128);
    assert_eq!(e2.status, 1);

    // Final settlement: 800,000,000
    let e3 = client.settle(&800_0000000i128);
    assert_eq!(e3.settled_amount, total_due);
    assert_eq!(e3.status, 2);
}

#[test]
#[should_panic(expected = "Settlement amount exceeds total due")]
fn test_over_settlement_failure() {
    let env = Env::default();
    env.mock_all_auths();

    let sme = Address::generate(&env);
    let admin = Address::generate(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &symbol_short!("INV021"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &2_000_0000000i128);
    client.fund(&investor, &3_000_0000000i128);
    assert_eq!(client.get_contribution(&investor), 5_000_0000000i128);
}

    client.init(
        &symbol_short!("INV_O1"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &2000u64,
        &test_hash(&env),
    );

    // Fund exactly the target in one shot
    let after_fund = client.fund(&investor, &10_000_0000000i128);
    assert_eq!(after_fund.funded_amount, 10_000_0000000i128);
    assert_eq!(after_fund.status, 1, "should be funded");

    // Settle
    let after_settle = client.settle();
    assert_eq!(after_settle.status, 2, "should be settled");
}

#[test]
fn test_partial_funding_multiple_investors() {
    let (env, client) = setup();
    let sme = Address::generate(&env);
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let inv_c = Address::generate(&env);

    client.init(
        &symbol_short!("INV003"),
        &sme,
        &9_000_0000000i128,
        &500i64,
        &3000u64,
    );

    // Three partial contributions
    let s1 = client.fund(&inv_a, &3_000_0000000i128);
    assert_eq!(s1.status, 0, "still open after first tranche");

    let s2 = client.fund(&inv_b, &3_000_0000000i128);
    assert_eq!(s2.status, 0, "still open after second tranche");

    let s3 = client.fund(&inv_c, &3_000_0000000i128);
    assert_eq!(s3.funded_amount, 9_000_0000000i128);
    assert_eq!(s3.status, 1, "funded after third tranche completes target");
}

#[test]
fn test_overfunding_still_funded() {
    let (env, client) = setup();
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(
        &symbol_short!("INV004"),
        &sme,
        &5_000_0000000i128,
        &300i64,
        &4000u64,
    );

    // Fund more than the target
    let after = client.fund(&investor, &7_000_0000000i128);
    assert_eq!(after.funded_amount, 7_000_0000000i128);
    assert_eq!(after.status, 1, "over-funded escrow must still be status=1");
}

#[test]
fn test_init_field_integrity() {
    let (env, client) = setup();
    let sme = Address::generate(&env);

    let escrow = client.init(
        &symbol_short!("INV005"),
        &sme,
        &1_500_0000000i128,
        &1200i64,
        &9999u64,
    );

    // funding_target must mirror amount
    assert_eq!(escrow.funding_target, escrow.amount);
    // sme_address must be preserved
    assert_eq!(escrow.sme_address, sme);
}

#[test]
fn test_yield_bps_stored() {
    let (env, client) = setup();
    let sme = Address::generate(&env);

#[test]
#[should_panic(expected = "Escrow must be funded or withdrawn before settlement")]
fn test_settle_before_funded_panics() {
    let (_, client, admin, sme) = setup();
    client.init(
        &symbol_short!("INV006"),
        &sme,
        &1_000_0000000i128,
        &1500i64, // 15%
        &5000u64,
    );

    assert_eq!(client.get_escrow().yield_bps, 1500);
}

#[test]
fn test_maturity_stored() {
    let (env, client) = setup();
    let sme = Address::generate(&env);

    client.init(
        &symbol_short!("INV007"),
        &sme,
        &1_000_0000000i128,
        &800i64,
        &u64::MAX,
    );

    assert_eq!(client.get_escrow().maturity, u64::MAX);
}

#[test]
fn test_minimum_amount_escrow() {
    let (env, client) = setup();
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(&symbol_short!("INV008"), &sme, &1i128, &0i64, &1u64);

    let after = client.fund(&investor, &1i128);
    assert_eq!(after.status, 1);

    let settled = client.settle();
    assert_eq!(settled.status, 2);
}

#[test]
fn test_zero_amount_fund_no_status_change() {
    let (env, client) = setup();
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(
        &symbol_short!("INV009"),
        &sme,
        &1_000_0000000i128,
        &800i64,
        &1000u64,
    );

    // A zero-amount fund call should not flip status
    let after = client.fund(&investor, &0i128);
    assert_eq!(after.status, 0, "zero-amount fund must not change status");
    assert_eq!(after.funded_amount, 0);
}

// ---------------------------------------------------------------------------
// Failure / panic tests
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Escrow not open for funding")]
fn test_fund_after_funded_panics() {
    let (env, client) = setup();
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    client.init(
        &symbol_short!("INV010"),
        &sme,
        &1_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &1_000_0000000i128); // reaches status=1
    client.fund(&investor, &1i128); // must panic
}

    client.fund(&investor, &10_000_0000000i128);

    let interest = (10_000_0000000i128 * 800) / 10000;
    let total_due = 10_000_0000000i128 + interest;

    // Try to settle more than due
    client.settle(&(total_due + 1));
}

#[test]
#[should_panic(expected = "Escrow must be funded before settlement")]
fn test_settle_not_funded() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let admin = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    client.init(
        &symbol_short!("INV_NF"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    // Not funded, should panic
    client.settle(&1000i128);
}

#[test]
fn test_update_maturity_success() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let admin = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);
    client.init(
        &admin,
        &symbol_short!("INV006"),
        &sme,
        &1_000i128,
        &500i64,
        &0u64,
    );
    client.fund(&investor, &1_000i128);
    client.settle();
    let auths = env.auths();
    assert!(
        auths.iter().any(|(addr, _)| *addr == sme),
        "sme auth not recorded"
    );
}

#[test]
#[should_panic]
fn test_settle_unauthorized_panics() {
    let (env, client, admin, sme) = setup();
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &symbol_short!("INV008"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &1_000i128);
    env.mock_auths(&[]);
    client.settle();
}

    let new_maturity = 2000u64;
    let escrow = client.update_maturity(&new_maturity);
    assert_eq!(escrow.maturity, new_maturity);

    // Verify state is still Open
    assert_eq!(escrow.status, 0);
}

#[test]
#[should_panic]
fn test_update_maturity_unauthorized() {
    let (env, client, admin, sme) = setup();
    client.init(
        &admin,
        &symbol_short!("INV009"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    env.mock_auths(&[]);
    client.update_maturity(&2000u64);
}

#[test]
fn test_cost_baseline_fund_partial() {
    let (env, client, admin, sme) = setup();
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &symbol_short!("INV001"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &5_000_0000000i128);
    client.fund(&investor, &5_000_0000000i128);
}

#[test]
#[should_panic(expected = "Maturity can only be updated in Open state")]
fn test_update_maturity_wrong_state() {
    let (env, client, admin, sme) = setup();
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &symbol_short!("INV010"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &5_000_0000000i128);
    client.fund(&investor, &5_000_0000000i128);
}

#[test]
fn test_cost_baseline_settle() {
    let (env, client, admin, sme) = setup();
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &symbol_short!("INV001"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
        &test_hash(&env),
    );
    client.fund(&investor, &10_000_0000000i128);
    env.ledger().set_timestamp(1001);
    client.settle();
}

    // Fund the escrow to change state to 1 (Funded)
    client.fund(&investor, &10_000_0000000i128);

    let escrow = client.get_escrow();
    assert_eq!(escrow.status, 1);

    // This should panic
    client.update_maturity(&2000u64);
}

#[test]
fn test_full_funding_updates_status() {
    let env = Env::default();
    let (client, sme, id) = setup_test(&env);
    let investor = Address::generate(&env);

    client.init(&id, &sme, &1000, &800, &10000);
    client.fund(&investor, &1000);

    let escrow = client.get_escrow();
    assert_eq!(escrow.status, 1); // Status 1 = Funded
}

/// Read-only methods are never blocked by pause state.
#[test]
fn test_read_only_methods_unaffected_by_pause() {
    let env = Env::default();
    let state = paused_state();

    let v = EscrowContract::version(&env).to_string();
    assert!(!v.is_empty());

    let paused = EscrowContract::is_paused(&state);
    assert!(paused);

    let escrow = default_escrow();
    let read = EscrowContract::get_escrow(&escrow);
    assert_eq!(read.invoice_id, 42);
}

/// Edge Case: Partial fund then full fund leads to funded
#[test]
fn test_transition_partial_then_full_funded() {
    let (env, client, admin, sme) = setup();
    let investor = Address::generate(&env);

    client.init(&admin, &symbol_short!("TX011"), &sme, &1000i128, &500i64, &2000u64);
    assert_eq!(client.get_escrow().status, 0);

    let escrow = client.fund(&investor, &500i128); // partial
    assert_eq!(escrow.status, 0); // still open
    assert_eq!(escrow.funded_amount, 500i128);

    let escrow = client.fund(&investor, &500i128); // complete funding
    assert_eq!(escrow.status, 1); // funded
}

/// Edge Case: Multiple partial funds without reaching target
#[test]
fn test_transition_multiple_partial_funds() {
    let (env, client, admin, sme) = setup();
    let investor1 = Address::generate(&env);
    let investor2 = Address::generate(&env);

    client.init(&admin, &symbol_short!("TX012"), &sme, &1000i128, &500i64, &2000u64);

    client.fund(&investor1, &300i128); // status = 0 (open)
    let escrow = client.fund(&investor2, &300i128); // still open, 600 funded
    assert_eq!(escrow.status, 0);
    assert_eq!(escrow.funded_amount, 600i128);

    client.fund(&investor1, &400i128); // now 1000 reached -> funded
    assert_eq!(client.get_escrow().status, 1);
}

/// Security: Verify status values are exactly as defined in matrix
#[test]
fn test_state_values_are_correct() {
    let (env, client, admin, sme) = setup();
    let investor = Address::generate(&env);

    client.init(&admin, &symbol_short!("TX013"), &sme, &1000i128, &500i64, &2000u64);
    let escrow = client.get_escrow();
    assert_eq!(escrow.status, 0, "Init should set status to Open (0)");

    client.fund(&investor, &1000i128);
    let escrow = client.get_escrow();
    assert_eq!(escrow.status, 1, "Full funding should set status to Funded (1)");

    client.settle();
    let escrow = client.get_escrow();
    assert_eq!(escrow.status, 2, "Settle should set status to Settled (2)");
}

// ---------------------------------------------------------------------------
// EscrowFactory tests
// ---------------------------------------------------------------------------

/// Helper: deploy a fresh EscrowFactory and return (env, client, admin, sme).
fn factory_setup() -> (Env, EscrowFactoryClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let contract_id = env.register(EscrowFactory, ());
    let client = EscrowFactoryClient::new(&env, &contract_id);
    (env, client, admin, sme)
}

/// create_escrow stores the escrow and it is retrievable via get_escrow.
#[test]
fn test_factory_create_and_get_escrow() {
    let (_, client, admin, sme) = factory_setup();

    let escrow = client.create_escrow(
        &admin,
        &symbol_short!("F001"),
        &sme,
        &10_000i128,
        &800i64,
        &1000u64,
    );

    assert_eq!(escrow.invoice_id, symbol_short!("F001"));
    assert_eq!(escrow.admin, admin);
    assert_eq!(escrow.sme_address, sme);
    assert_eq!(escrow.amount, 10_000i128);
    assert_eq!(escrow.funded_amount, 0);
    assert_eq!(escrow.status, 0);

    let got = client.get_escrow(&symbol_short!("F001"));
    assert_eq!(got.invoice_id, escrow.invoice_id);
    assert_eq!(got.admin, admin);
}

/// Factory isolates multiple escrows — each invoice is independent.
#[test]
fn test_factory_multiple_escrows_isolated() {
    let (env, client, admin, sme) = factory_setup();
    let sme2 = Address::generate(&env);

    client.create_escrow(
        &admin,
        &symbol_short!("F002"),
        &sme,
        &1_000i128,
        &500i64,
        &500u64,
    );
    client.create_escrow(
        &admin,
        &symbol_short!("F003"),
        &sme2,
        &2_000i128,
        &600i64,
        &600u64,
    );

    let e1 = client.get_escrow(&symbol_short!("F002"));
    let e2 = client.get_escrow(&symbol_short!("F003"));

    // Each escrow holds its own state independently.
    assert_eq!(e1.amount, 1_000i128);
    assert_eq!(e2.amount, 2_000i128);
    assert_eq!(e1.sme_address, sme);
    assert_eq!(e2.sme_address, sme2);
}

/// list_invoices returns all invoice IDs in creation order.
#[test]
fn test_factory_list_invoices() {
    let (_, client, admin, sme) = factory_setup();

    assert_eq!(client.list_invoices().len(), 0);

    client.create_escrow(&admin, &symbol_short!("F004"), &sme, &1_000i128, &500i64, &500u64);
    client.create_escrow(&admin, &symbol_short!("F005"), &sme, &2_000i128, &600i64, &600u64);

    let list = client.list_invoices();
    assert_eq!(list.len(), 2);
    assert_eq!(list.get(0).unwrap(), symbol_short!("F004"));
    assert_eq!(list.get(1).unwrap(), symbol_short!("F005"));
}

/// fund via factory updates funded_amount and flips status when target met.
#[test]
fn test_factory_fund_partial_then_full() {
    let (env, client, admin, sme) = factory_setup();
    let investor = Address::generate(&env);

    client.create_escrow(&admin, &symbol_short!("F006"), &sme, &1_000i128, &500i64, &500u64);

    let e1 = client.fund(&symbol_short!("F006"), &investor, &400i128);
    assert_eq!(e1.funded_amount, 400i128);
    assert_eq!(e1.status, 0);

    let e2 = client.fund(&symbol_short!("F006"), &investor, &600i128);
    assert_eq!(e2.funded_amount, 1_000i128);
    assert_eq!(e2.status, 1);
}

/// settle via factory transitions status from funded (1) to settled (2).
#[test]
fn test_factory_settle_after_full_funding() {
    let (env, client, admin, sme) = factory_setup();
    let investor = Address::generate(&env);

    client.create_escrow(&admin, &symbol_short!("F007"), &sme, &1_000i128, &500i64, &500u64);
    client.fund(&symbol_short!("F007"), &investor, &1_000i128);
    let escrow = client.settle(&symbol_short!("F007"));
    assert_eq!(escrow.status, 2);
}

/// Duplicate invoice_id must be rejected.
#[test]
#[should_panic(expected = "Escrow already exists for invoice")]
fn test_factory_duplicate_invoice_panics() {
    let (_, client, admin, sme) = factory_setup();

    client.create_escrow(&admin, &symbol_short!("F008"), &sme, &1_000i128, &500i64, &500u64);
    client.create_escrow(&admin, &symbol_short!("F008"), &sme, &2_000i128, &500i64, &500u64);
}

/// get_escrow for an unknown invoice must panic.
#[test]
#[should_panic(expected = "Escrow not found for invoice")]
fn test_factory_get_unknown_invoice_panics() {
    let (_, client, _, _) = factory_setup();
    client.get_escrow(&symbol_short!("NOPE"));
}

/// fund on a funded escrow must panic.
#[test]
#[should_panic(expected = "Escrow not open for funding")]
fn test_factory_fund_after_funded_panics() {
    let (env, client, admin, sme) = factory_setup();
    let investor = Address::generate(&env);

    client.create_escrow(&admin, &symbol_short!("F009"), &sme, &1_000i128, &500i64, &500u64);
    client.fund(&symbol_short!("F009"), &investor, &1_000i128);
    client.fund(&symbol_short!("F009"), &investor, &1i128); // should panic
}

/// settle on an unfunded escrow must panic.
#[test]
#[should_panic(expected = "Escrow must be funded")]
fn test_factory_settle_unfunded_panics() {
    let (_, client, admin, sme) = factory_setup();
    client.create_escrow(&admin, &symbol_short!("F010"), &sme, &1_000i128, &500i64, &500u64);
    client.settle(&symbol_short!("F010"));
}

/// fund on one invoice must not affect a different invoice's state.
#[test]
fn test_factory_fund_does_not_bleed_across_invoices() {
    let (env, client, admin, sme) = factory_setup();
    let investor = Address::generate(&env);

    client.create_escrow(&admin, &symbol_short!("FA01"), &sme, &1_000i128, &500i64, &500u64);
    client.create_escrow(&admin, &symbol_short!("FA02"), &sme, &2_000i128, &600i64, &600u64);

    client.fund(&symbol_short!("FA01"), &investor, &500i128);

    let e1 = client.get_escrow(&symbol_short!("FA01"));
    let e2 = client.get_escrow(&symbol_short!("FA02"));
    assert_eq!(e1.funded_amount, 500i128);
    assert_eq!(e2.funded_amount, 0i128, "FA02 must be unaffected by FA01 funding");
}

/// create_escrow requires admin auth.
#[test]
fn test_factory_create_requires_admin_auth() {
    let (env, client, admin, sme) = factory_setup();

    client.create_escrow(&admin, &symbol_short!("FA03"), &sme, &1_000i128, &500i64, &500u64);

    assert!(
        env.auths().iter().any(|(addr, _)| *addr == admin),
        "admin auth not recorded for create_escrow"
    );
}

/// fund via factory requires investor auth.
#[test]
fn test_factory_fund_requires_investor_auth() {
    let (env, client, admin, sme) = factory_setup();
    let investor = Address::generate(&env);

    client.create_escrow(&admin, &symbol_short!("FA04"), &sme, &1_000i128, &500i64, &500u64);
    client.fund(&symbol_short!("FA04"), &investor, &500i128);

    assert!(
        env.auths().iter().any(|(addr, _)| *addr == investor),
        "investor auth not recorded for factory fund"
    );
}

/// settle via factory requires SME auth.
#[test]
fn test_factory_settle_requires_sme_auth() {
    let (env, client, admin, sme) = factory_setup();
    let investor = Address::generate(&env);

    client.create_escrow(&admin, &symbol_short!("FA05"), &sme, &1_000i128, &500i64, &500u64);
    client.fund(&symbol_short!("FA05"), &investor, &1_000i128);
    client.settle(&symbol_short!("FA05"));

    assert!(
        env.auths().iter().any(|(addr, _)| *addr == sme),
        "sme auth not recorded for factory settle"
    );
}

#[test]
fn test_transfer_admin_updates_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV_TA"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    let escrow = client.transfer_admin(&new_admin);
    assert_eq!(escrow.admin, new_admin);
}

#[test]
fn test_transfer_admin_records_new_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV_AR"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    client.transfer_admin(&new_admin);

    assert!(
        env.auths().iter().any(|(addr, _)| *addr == admin),
        "current admin auth not recorded for transfer_admin"
    );
}

#[test]
fn test_transfer_admin_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV_AE"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    client.transfer_admin(&new_admin);

    let events = env.events().all();
    assert!(
        !events.is_empty(),
        "expected at least one event after transfer_admin"
    );
}

#[test]
#[should_panic(expected = "New admin must differ from current admin")]
fn test_transfer_admin_same_address_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV_SA"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    client.transfer_admin(&admin);
}

#[test]
#[should_panic(expected = "Escrow not initialized")]
fn test_transfer_admin_uninitialized_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let new_admin = Address::generate(&env);
    let client = deploy(&env);

    client.transfer_admin(&new_admin);
}

#[test]
#[should_panic]
fn test_transfer_admin_unauthorized_panics() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let malicious = Address::generate(&env);

    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(
        &admin,
        &symbol_short!("INV_UA"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    env.mock_auths(&[]);
    client.transfer_admin(&malicious);
}

#[test]
fn test_transfer_admin_chained_rotation() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admin3 = Address::generate(&env);
    let client = deploy(&env);

    client.init(
        &admin,
        &symbol_short!("INV_CH"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    let e1 = client.transfer_admin(&admin2);
    assert_eq!(e1.admin, admin2);

    let e2 = client.transfer_admin(&admin3);
    assert_eq!(e2.admin, admin3);
}

#[test]
fn test_transfer_admin_preserves_escrow_fields() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let client = deploy(&env);

    let before = client.init(
        &admin,
        &symbol_short!("INV_PF"),
        &sme,
        &admin,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    let after = client.transfer_admin(&new_admin);

    assert_eq!(after.invoice_id, before.invoice_id);
    assert_eq!(after.sme_address, before.sme_address);
    assert_eq!(after.amount, before.amount);
    assert_eq!(after.funded_amount, before.funded_amount);
    assert_eq!(after.yield_bps, before.yield_bps);
    assert_eq!(after.maturity, before.maturity);
    assert_eq!(after.status, before.status);
}

#[test]
fn test_factory_transfer_admin_updates_admin() {
    let (env, client, admin, sme) = factory_setup();
    let new_admin = Address::generate(&env);

    client.create_escrow(&admin, &symbol_short!("G001"), &sme, &1_000i128, &500i64, &500u64);
    let escrow = client.transfer_admin(&symbol_short!("G001"), &new_admin);
    assert_eq!(escrow.admin, new_admin);
}

#[test]
fn test_factory_transfer_admin_isolated() {
    let (env, client, admin, sme) = factory_setup();
    let new_admin = Address::generate(&env);

    client.create_escrow(&admin, &symbol_short!("G002"), &sme, &1_000i128, &500i64, &500u64);
    client.create_escrow(&admin, &symbol_short!("G003"), &sme, &1_000i128, &500i64, &500u64);

    client.transfer_admin(&symbol_short!("G002"), &new_admin);
    let g2 = client.get_escrow(&symbol_short!("G002"));
    let g3 = client.get_escrow(&symbol_short!("G003"));
    assert_eq!(g2.admin, new_admin);
    assert_eq!(g3.admin, admin, "G003 admin must be unchanged");
}

/// Factory transfer_admin to same address must panic.
#[test]
#[should_panic(expected = "New admin must differ from current admin")]
fn test_factory_transfer_admin_same_address_panics() {
    let (_, client, admin, sme) = factory_setup();

    client.create_escrow(&admin, &symbol_short!("G004"), &sme, &1_000i128, &500i64, &500u64);
    client.transfer_admin(&symbol_short!("G004"), &admin);
}

/// Factory transfer_admin on unknown invoice must panic.
#[test]
#[should_panic(expected = "Escrow not found for invoice")]
fn test_factory_transfer_admin_unknown_invoice_panics() {
    let (env, client, _, _) = factory_setup();
    let new_admin = Address::generate(&env);

    client.transfer_admin(&symbol_short!("NOPE"), &new_admin);
}

/// Factory transfer_admin emits event.
#[test]
fn test_factory_transfer_admin_emits_event() {
    let (env, client, admin, sme) = factory_setup();
    let new_admin = Address::generate(&env);

    client.create_escrow(&admin, &symbol_short!("G005"), &sme, &1_000i128, &500i64, &500u64);
    client.transfer_admin(&symbol_short!("G005"), &new_admin);

    let events = env.events().all();
    assert!(
        !events.is_empty(),
        "expected at least one event after factory transfer_admin"
    );
}

── END LEGACY ────────────────────────────────────────────────────────────── */
