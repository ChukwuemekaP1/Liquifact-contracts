use super::*;

// Funding, contributions, snapshots, tier selection, and fund-shaped cost baselines.

#[test]
fn test_fund_and_settle() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "INVMETA"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
    );
    let funded = client.fund(&investor, &TARGET);
    assert_eq!(funded.funded_amount, TARGET);
    assert_eq!(funded.status, 1);
    let settled = client.settle();
    assert_eq!(settled.status, 2);
}

#[test]
fn test_fund_partial_then_full() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "INV002"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
    );
    let partial = client.fund(&investor, &(TARGET / 2));
    assert_eq!(partial.status, 0);
    assert_eq!(partial.funded_amount, TARGET / 2);
    let full = client.fund(&investor, &(TARGET / 2));
    assert_eq!(full.status, 1);
    assert_eq!(full.funded_amount, TARGET);
}

#[test]
#[should_panic(expected = "Funding amount must be positive")]
fn test_fund_zero_amount_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &0i128);
}

#[test]
#[should_panic(expected = "Escrow not open for funding")]
fn test_fund_after_funded_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &TARGET);
    client.fund(&investor, &1i128);
}

#[test]
fn test_fund_requires_investor_auth() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &TARGET);
    assert!(
        env.auths().iter().any(|(addr, _)| *addr == investor),
        "investor auth was not recorded for fund"
    );
}

#[test]
fn test_single_investor_contribution_tracked() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "INV020"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
    );
    client.fund(&investor, &(3_000_0000000i128));
    let contribution = client.get_contribution(&investor);
    assert_eq!(contribution, 3_000_0000000i128);
}

#[test]
fn test_unknown_investor_contribution_is_zero() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let stranger = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &1_000i128);
    assert_eq!(client.get_contribution(&stranger), 0i128);
}

#[test]
fn test_repeated_funding_accumulates_contribution() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "INV021"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
    );
    client.fund(&investor, &(2_000_0000000i128));
    client.fund(&investor, &(3_000_0000000i128));
    assert_eq!(client.get_contribution(&investor), 5_000_0000000i128);
}

#[test]
fn test_multiple_investors_tracked_independently() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let inv_c = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "INV023"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
    );
    client.fund(&inv_a, &(2_000_0000000i128));
    client.fund(&inv_b, &(5_000_0000000i128));
    client.fund(&inv_c, &(3_000_0000000i128));
    assert_eq!(client.get_contribution(&inv_a), 2_000_0000000i128);
    assert_eq!(client.get_contribution(&inv_b), 5_000_0000000i128);
    assert_eq!(client.get_contribution(&inv_c), 3_000_0000000i128);
    let sum = client.get_contribution(&inv_a)
        + client.get_contribution(&inv_b)
        + client.get_contribution(&inv_c);
    assert_eq!(sum, client.get_escrow().funded_amount);
}

#[test]
fn test_contributions_sum_equals_funded_amount() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let inv_c = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "INV023b"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
    );
    client.fund(&inv_a, &(2_000_0000000i128));
    client.fund(&inv_b, &(5_000_0000000i128));
    client.fund(&inv_c, &(3_000_0000000i128));
    let sum = client.get_contribution(&inv_a)
        + client.get_contribution(&inv_b)
        + client.get_contribution(&inv_c);
    assert_eq!(sum, client.get_escrow().funded_amount);
}

#[test]
fn test_cost_baseline_fund_partial() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "INV103"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
    );
    client.fund(&investor, &(1_000_0000000i128));
}

#[test]
fn test_cost_baseline_fund_full() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "INV104"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
    );
    client.fund(&investor, &TARGET);
}

#[test]
fn test_cost_baseline_fund_overshoot() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "INV105"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
    );
    client.fund(&investor, &(15_000_0000000i128));
    assert_eq!(client.get_escrow().status, 1);
}

#[test]
fn test_cost_baseline_fund_two_step_completion() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "INV106"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
    );
    client.fund(&investor, &(TARGET / 2));
    client.fund(&investor, &(TARGET / 2));
    assert_eq!(client.get_escrow().status, 1);
}

#[test]
fn test_funding_close_snapshot_captures_overfunded_total_once() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &String::from_str(&env, "SNAP001"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
    );
    assert_eq!(client.get_funding_close_snapshot(), None);
    client.fund(&inv, &(TARGET + 5_000_0000000i128));
    let snap = client.get_funding_close_snapshot().expect("snapshot");
    assert_eq!(snap.total_principal, TARGET + 5_000_0000000i128);
    assert_eq!(snap.funding_target, TARGET);
    assert_eq!(snap.closed_at_ledger_timestamp, env.ledger().timestamp());
    assert_eq!(snap.closed_at_ledger_sequence, env.ledger().sequence());
}

#[test]
fn test_funding_snapshot_immutable_across_second_fund_after_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &String::from_str(&env, "SNAP002"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
    );
    client.fund(&a, &(TARGET / 2));
    assert_eq!(client.get_funding_close_snapshot(), None);
    client.fund(&b, &(TARGET / 2));
    let s1 = client.get_funding_close_snapshot().unwrap();
    assert_eq!(s1.total_principal, TARGET);
    let s2 = client.get_funding_close_snapshot().unwrap();
    assert_eq!(s1, s2);
}

#[test]
fn test_pro_rata_weight_ratio_from_snapshot() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &String::from_str(&env, "SNAP003"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
    );
    client.fund(&a, &(2_000_0000000i128));
    client.fund(&b, &(8_000_0000000i128));
    let snap = client.get_funding_close_snapshot().unwrap();
    assert_eq!(snap.total_principal, TARGET);
    let ca = client.get_contribution(&a);
    let cb = client.get_contribution(&b);
    assert_eq!(ca + cb, snap.total_principal);
}

#[test]
fn test_tiered_yield_and_follow_on_fund() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 900,
    });
    tiers.push_back(YieldTier {
        min_lock_secs: 500,
        yield_bps: 1100,
    });
    client.init(
        &admin,
        &String::from_str(&env, "TIER001"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
    );
    client.fund_with_commitment(&inv, &5_000i128, &200u64);
    assert_eq!(client.get_investor_yield_bps(&inv), 900);
    assert_eq!(client.get_investor_claim_not_before(&inv), 200);
    client.fund(&inv, &5_000i128);
    assert_eq!(client.get_investor_yield_bps(&inv), 900);
    assert_eq!(client.get_escrow().status, 1);
}

#[test]
fn test_tier_selection_edges_base_vs_high_bucket() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let i_short = Address::generate(&env);
    let i_long = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 50,
        yield_bps: 850,
    });
    client.init(
        &admin,
        &String::from_str(&env, "TIER002"),
        &sme,
        &20_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
    );
    client.fund_with_commitment(&i_short, &10_000i128, &40u64);
    assert_eq!(client.get_investor_yield_bps(&i_short), 800);
    client.fund_with_commitment(&i_long, &10_000i128, &50u64);
    assert_eq!(client.get_investor_yield_bps(&i_long), 850);
}

#[test]
#[should_panic(expected = "Additional principal after a tiered first deposit")]
fn test_fund_with_commitment_twice_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 1,
        yield_bps: 810,
    });
    client.init(
        &admin,
        &String::from_str(&env, "TIER003"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
    );
    client.fund_with_commitment(&inv, &5_000i128, &10u64);
    client.fund_with_commitment(&inv, &5_000i128, &10u64);
}

#[test]
#[should_panic(expected = "strictly increasing min_lock_secs")]
fn test_init_bad_tier_order_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 200,
        yield_bps: 900,
    });
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 950,
    });
    client.init(
        &admin,
        &String::from_str(&env, "BADTIER"),
        &sme,
        &1_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
    );
}

#[test]
#[should_panic(expected = "tier yield_bps must be >= base yield_bps")]
fn test_init_tier_yield_below_base_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 10,
        yield_bps: 700,
    });
    client.init(
        &admin,
        &String::from_str(&env, "BADT2"),
        &sme,
        &1_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
    );
}

#[test]
fn test_differential_funding_target_eq_exact_cross() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let t = 5_000i128;
    client.init(
        &admin,
        &String::from_str(&env, "DIFF002"),
        &sme,
        &t,
        &100i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
    );
    let escrow = client.fund(&inv, &t);
    assert_eq!(escrow.funded_amount, t);
    assert_eq!(escrow.status, 1);
    let snap = client.get_funding_close_snapshot().unwrap();
    assert_eq!(snap.total_principal, t);
    assert_eq!(snap.funding_target, t);
}

#[test]
fn test_ledger_sequence_recorded_in_snapshot_with_tick() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &String::from_str(&env, "DIFF003"),
        &sme,
        &1_000i128,
        &100i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
    );
    let seq = env.ledger().sequence();
    client.fund(&inv, &1_000i128);
    let snap = client.get_funding_close_snapshot().unwrap();
    assert_eq!(snap.closed_at_ledger_sequence, seq);
}

// --- min_contribution_floor funding regression tests ---

/// Helper: init with a floor and return (client, investor).
fn init_with_floor(
    env: &Env,
    invoice_id: &str,
    target: i128,
    floor: i128,
) -> (LiquifactEscrowClient<'_>, Address) {
    let client = deploy(env);
    let admin = Address::generate(env);
    let sme = Address::generate(env);
    let (tok, tre) = free_addresses(env);
    client.init(
        &admin,
        &String::from_str(env, invoice_id),
        &sme,
        &target,
        &500i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &Some(floor),
        &None,
    );
    let investor = Address::generate(env);
    (client, investor)
}

/// `get_min_contribution_floor` returns the configured floor.
#[test]
fn test_get_min_contribution_floor_returns_configured_value() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = init_with_floor(&env, "FL001", 10_000i128, 500i128);
    assert_eq!(client.get_min_contribution_floor(), 500i128);
}

/// Funding exactly at the floor succeeds.
#[test]
fn test_fund_at_floor_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, investor) = init_with_floor(&env, "FL002", 10_000i128, 500i128);
    let escrow = client.fund(&investor, &500i128);
    assert_eq!(client.get_contribution(&investor), 500i128);
    assert_eq!(escrow.funded_amount, 500i128);
}

/// Funding one unit below the floor panics.
#[test]
#[should_panic(expected = "funding amount below min_contribution floor")]
fn test_fund_below_floor_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, investor) = init_with_floor(&env, "FL003", 10_000i128, 500i128);
    client.fund(&investor, &499i128);
}

/// Funding one unit above the floor succeeds.
#[test]
fn test_fund_one_above_floor_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, investor) = init_with_floor(&env, "FL004", 10_000i128, 500i128);
    client.fund(&investor, &501i128);
    assert_eq!(client.get_contribution(&investor), 501i128);
}

/// The floor applies to every call, not just the first deposit.
/// A follow-on deposit below the floor from the same investor must panic.
#[test]
#[should_panic(expected = "funding amount below min_contribution floor")]
fn test_fund_follow_on_below_floor_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, investor) = init_with_floor(&env, "FL005", 10_000i128, 500i128);
    client.fund(&investor, &500i128); // first deposit — OK
    client.fund(&investor, &499i128); // follow-on below floor — must panic
}

/// A follow-on deposit at the floor from the same investor succeeds and accumulates.
#[test]
fn test_fund_follow_on_at_floor_accumulates() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, investor) = init_with_floor(&env, "FL006", 10_000i128, 500i128);
    client.fund(&investor, &500i128);
    client.fund(&investor, &500i128);
    assert_eq!(client.get_contribution(&investor), 1_000i128);
}

/// When no floor is configured (None), any positive amount is accepted.
#[test]
fn test_fund_no_floor_accepts_one_unit() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &1i128);
    assert_eq!(client.get_contribution(&investor), 1i128);
}

/// Over-funding past the target with a floor: the single call that crosses the target
/// must still meet the floor, and the snapshot captures the over-funded total.
#[test]
fn test_fund_overshoot_with_floor_records_snapshot() {
    let env = Env::default();
    env.mock_all_auths();
    let target = 5_000i128;
    let floor = 1_000i128;
    let (client, investor) = init_with_floor(&env, "FL007", target, floor);
    // Single call: above floor, above target → funded immediately
    let escrow = client.fund(&investor, &6_000i128);
    assert_eq!(escrow.status, 1);
    assert_eq!(escrow.funded_amount, 6_000i128);
    let snap = client.get_funding_close_snapshot().unwrap();
    assert_eq!(snap.total_principal, 6_000i128);
    assert_eq!(snap.funding_target, target);
}

/// Two investors each contributing exactly the floor reach the target together;
/// contributions sum to funded_amount and snapshot is correct.
#[test]
fn test_fund_two_investors_at_floor_reach_target() {
    let env = Env::default();
    env.mock_all_auths();
    let target = 2_000i128;
    let floor = 1_000i128;
    let (client, inv_a) = init_with_floor(&env, "FL008", target, floor);
    let inv_b = Address::generate(&env);
    client.fund(&inv_a, &floor);
    let escrow = client.fund(&inv_b, &floor);
    assert_eq!(escrow.status, 1);
    assert_eq!(escrow.funded_amount, target);
    assert_eq!(
        client.get_contribution(&inv_a) + client.get_contribution(&inv_b),
        target
    );
    let snap = client.get_funding_close_snapshot().unwrap();
    assert_eq!(snap.total_principal, target);
}

/// `fund_with_commitment` also enforces the floor on the first deposit.
#[test]
#[should_panic(expected = "funding amount below min_contribution floor")]
fn test_fund_with_commitment_below_floor_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, investor) = init_with_floor(&env, "FL009", 10_000i128, 500i128);
    client.fund_with_commitment(&investor, &499i128, &0u64);
}

/// `fund_with_commitment` at the floor succeeds and records the contribution.
#[test]
fn test_fund_with_commitment_at_floor_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, investor) = init_with_floor(&env, "FL010", 10_000i128, 500i128);
    client.fund_with_commitment(&investor, &500i128, &0u64);
    assert_eq!(client.get_contribution(&investor), 500i128);
}

/// Floor equal to the target: a single call at the floor funds the escrow exactly.
#[test]
fn test_fund_floor_equals_target_exact_fill() {
    let env = Env::default();
    env.mock_all_auths();
    let amount = 1_000i128;
    let (client, investor) = init_with_floor(&env, "FL011", amount, amount);
    let escrow = client.fund(&investor, &amount);
    assert_eq!(escrow.status, 1);
    assert_eq!(escrow.funded_amount, amount);
    let snap = client.get_funding_close_snapshot().unwrap();
    assert_eq!(snap.total_principal, amount);
    assert_eq!(snap.funding_target, amount);
}
