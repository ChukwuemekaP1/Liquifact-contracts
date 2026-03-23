use super::{
    EscrowFunded, EscrowInitialized, EscrowSettled, LiquifactEscrow, LiquifactEscrowClient,
};
use soroban_sdk::{
    symbol_short, testutils::Address as _, testutils::Events as _, Address, Env, Event,
};

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
fn test_init_and_get_escrow() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, client) = deploy(&env);
    let sme = Address::generate(&env);

    let escrow = client.init(
        &symbol_short!("INV001"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    assert_eq!(escrow.invoice_id, symbol_short!("INV001"));
    assert_eq!(escrow.amount, 10_000_0000000i128);
    assert_eq!(escrow.funding_target, 10_000_0000000i128);
    assert_eq!(escrow.funded_amount, 0);
    assert_eq!(escrow.yield_bps, 800);
    assert_eq!(escrow.maturity, 1000);
    assert_eq!(escrow.status, 0);

    // get_escrow must match what init returned
    let got = client.get_escrow();
    assert_eq!(got, escrow);
}

/// `init` must emit exactly one `EscrowInitialized` event whose payload
/// matches the returned snapshot.
///
/// `env.events().all()` captures events from the last invocation only — this
/// works perfectly since init is the only call in this test.
#[test]
fn test_init_emits_initialized_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy(&env);
    let sme = Address::generate(&env);

    let escrow = client.init(
        &symbol_short!("INV003"),
        &sme,
        &5_000_0000000i128,
        &600i64,
        &2000u64,
    );

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
    let investor = Address::generate(&env);

    client.init(
        &symbol_short!("INV002"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    // Partial contribution — status must stay open
    let partial = client.fund(&investor, &4_000_0000000i128);
    assert_eq!(partial.funded_amount, 4_000_0000000i128);
    assert_eq!(partial.status, 0);

    // Full contribution — status must become funded
    let full = client.fund(&investor, &6_000_0000000i128);
    assert_eq!(full.funded_amount, 10_000_0000000i128);
    assert_eq!(full.status, 1);
}

/// `fund` must emit an `EscrowFunded` event; when target is met the event
/// payload must reflect `status == 1`.
///
/// Note: `env.events().all()` returns events from the **last** contract
/// invocation only, so we assert immediately after the `fund()` call.
#[test]
fn test_fund_emits_funded_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(
        &symbol_short!("INV004"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );

    // Fully fund in one shot so status == 1 appears in the event payload.
    // Assert events immediately after this call.
    client.fund(&investor, &10_000_0000000i128);

    let expected_fund_event = EscrowFunded {
        name: symbol_short!("escrow_fd"),
        invoice_id: symbol_short!("INV004"),
        investor: investor.clone(),
        amount: 10_000_0000000i128,
        funded_amount: 10_000_0000000i128,
        status: 1,
    };

    // events().all() shows only the last invocation (fund) — exactly one event.
    assert_eq!(
        env.events().all(),
        std::vec![expected_fund_event.to_xdr(&env, &contract_id)],
        "EscrowFunded event payload mismatch"
    );
}

/// `fund` on an already-funded escrow (status 1) must panic.
#[test]
#[should_panic(expected = "Escrow not open for funding")]
fn test_fund_after_funded_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, client) = deploy(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(
        &symbol_short!("INV005"),
        &sme,
        &1_0000000i128,
        &500i64,
        &999u64,
    );
    client.fund(&investor, &1_0000000i128); // fully funds -> status 1
    client.fund(&investor, &1_0000000i128); // must panic
}

// ──────────────────────────────────────────────────────────────────────────────
// settle
// ──────────────────────────────────────────────────────────────────────────────

/// Full lifecycle: init -> fund -> settle; final status must be 2.
#[test]
fn test_fund_and_settle() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, client) = deploy(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(
        &symbol_short!("INV006"),
        &sme,
        &10_000_0000000i128,
        &800i64,
        &1000u64,
    );
    client.fund(&investor, &10_000_0000000i128);

    let settled = client.settle();
    assert_eq!(settled.status, 2);
}

/// `settle` must emit an `EscrowSettled` event carrying funded_amount,
/// yield_bps, and maturity for off-chain payout calculations.
///
/// Note: `env.events().all()` returns events from the **last** contract
/// invocation only, so we assert immediately after the `settle()` call.
#[test]
fn test_settle_emits_settled_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(
        &symbol_short!("INV007"),
        &sme,
        &20_000_0000000i128,
        &1000i64,
        &9999u64,
    );
    client.fund(&investor, &20_000_0000000i128);
    // Assert events immediately after settle() — last invocation shows only settle event.
    client.settle();

    let expected_settle_event = EscrowSettled {
        name: symbol_short!("escrow_sd"),
        invoice_id: symbol_short!("INV007"),
        funded_amount: 20_000_0000000i128,
        yield_bps: 1000,
        maturity: 9999,
    };

    assert_eq!(
        env.events().all(),
        std::vec![expected_settle_event.to_xdr(&env, &contract_id)],
        "EscrowSettled event payload mismatch"
    );
}

/// `settle` on an open (not yet funded) escrow must panic.
#[test]
#[should_panic(expected = "Escrow must be funded before settlement")]
fn test_settle_when_not_funded_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, client) = deploy(&env);
    let sme = Address::generate(&env);

    client.init(
        &symbol_short!("INV008"),
        &sme,
        &1_0000000i128,
        &500i64,
        &999u64,
    );
    client.settle(); // must panic -- escrow still open
}

/// `settle` on an already-settled escrow (status 2) must panic.
#[test]
#[should_panic(expected = "Escrow must be funded before settlement")]
fn test_double_settle_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, client) = deploy(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);

    client.init(
        &symbol_short!("INV009"),
        &sme,
        &1_0000000i128,
        &500i64,
        &999u64,
    );
    client.fund(&investor, &1_0000000i128);
    client.settle(); // first settle -- ok
    client.settle(); // second settle -- must panic (status is now 2, not 1)
}
