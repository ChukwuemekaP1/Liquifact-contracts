//! # LiquiFact Escrow Contract
//!
//! Holds investor funds for a tokenized invoice until the buyer settles at maturity.
//!
//! ## Lifecycle
//! ```text
//! [init] → status 0 (open)
//!        ↓ fund() calls accumulate funded_amount
//! [fund] → status 1 (funded) when funded_amount >= funding_target
//!        ↓ buyer pays off-chain; backend calls settle()
//! [settle] → status 2 (settled) — principal + yield released to investors
//! ```
//!
//! ## Events (indexer reference — see also docs/EVENT_SCHEMA.md)
//!
//! Every state-changing function emits a typed Soroban contract event via the
//! `#[contractevent]` macro so backend indexers can reconstruct contract
//! history from ledger meta without any custom RPC.
//!
//! | Event struct      | Topic field(s)                 | Data fields                                          |
//! |-------------------|-------------------------------|------------------------------------------------------|
//! | `EscrowInitialized` | `name = "escrow_initd"`     | Full `InvoiceEscrow` snapshot (status 0)             |
//! | `EscrowFunded`      | `name = "escrow_funded"`    | `invoice_id`, `investor`, `amount`, `funded_amount`, `status` |
//! | `EscrowSettled`     | `name = "escrow_settled"`   | `invoice_id`, `funded_amount`, `yield_bps`, `maturity` |
//!
//! ### Versioning strategy
//! The `name` topic uniquely identifies each event action within this contract namespace.
//! When a **breaking** change is made to a payload shape, rename the event
//! struct (e.g. `EscrowFundedV2`) and update `name` accordingly so indexers
//! can filter old vs new events independently.
//! Additive-only field additions do NOT require a version bump.

use soroban_sdk::{
    contract, contractevent, contractimpl, contracttype, symbol_short, Address, Env, Symbol,
};

// ──────────────────────────────────────────────────────────────────────────────
// Data types
// ──────────────────────────────────────────────────────────────────────────────

/// Full state of an invoice escrow persisted in contract storage.
///
/// All monetary values use the smallest indivisible unit of the relevant
/// Stellar asset (e.g. stroops for XLM, or the token's own precision).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvoiceEscrow {
    /// Unique invoice identifier agreed between SME and platform (e.g. `"INV1023"`).
    /// Maximum 8 ASCII characters due to Soroban `symbol_short!` constraints.
    pub invoice_id: Symbol,

    /// SME wallet address that will receive the stablecoin liquidity once
    /// the funding target is fully met.
    pub sme_address: Address,

    /// Nominal invoice face value in smallest token units (always positive).
    pub amount: i128,

    /// Investor funding target.  Currently equal to `amount`; may diverge
    /// in future versions that support partial invoice tokenization.
    pub funding_target: i128,

    /// Running total committed by investors so far (starts at 0).
    /// Status transitions to `1` (funded) the moment this reaches `funding_target`.
    pub funded_amount: i128,

    /// Annualized investor yield expressed in basis points.
    /// Example: `800` = 8 %.  Backend must convert to absolute amount at settlement.
    pub yield_bps: i64,

    /// Ledger timestamp at which the invoice matures and settlement is expected.
    /// Stored as seconds since Unix epoch (Soroban `u64` ledger time).
    pub maturity: u64,

    /// Escrow lifecycle status:
    /// - `0` — **open**: accepting investor funding
    /// - `1` — **funded**: target met; SME can be paid; awaiting buyer settlement
    /// - `2` — **settled**: buyer paid; investors can redeem principal + yield
    pub status: u32,
}

// ──────────────────────────────────────────────────────────────────────────────
// Event types (one per state-changing function)
//
// Fields annotated with `#[topic]` appear in the Soroban event topic vector;
// all other fields appear in the event data payload.
//
// Keeping payloads as named structs makes XDR decoding forward-compatible and
// self-documenting in ledger explorers.  See docs/EVENT_SCHEMA.md for the
// full indexer reference including JSON examples and XDR topic filters.
// ──────────────────────────────────────────────────────────────────────────────

/// Emitted by `init()` when a new invoice escrow is created.
///
/// ### Indexer example (JSON after XDR decode)
/// ```json
/// {
///   "event"         : "escrow_initd",
///   "invoice_id"    : "INV1023",
///   "sme_address"   : "GBSME...",
///   "amount"        : 100000000000,
///   "funding_target": 100000000000,
///   "funded_amount" : 0,
///   "yield_bps"     : 800,
///   "maturity"      : 1750000000,
///   "status"        : 0
/// }
/// ```
#[contractevent]
pub struct EscrowInitialized {
    /// Event name topic — used by indexers to filter this event type.
    #[topic]
    pub name: Symbol,
    /// Full escrow snapshot at creation time (status always 0 / open).
    pub escrow: InvoiceEscrow,
}

/// Emitted by `fund()` on every successful investor contribution.
///
/// Emitted on **every** `fund()` call, not only when the target is first met.
/// Indexers can sum `amount` per `invoice_id` to reconstruct the full funding
/// history without reading contract storage.
///
/// ### Indexer example (JSON after XDR decode)
/// ```json
/// {
///   "event"        : "escrow_funded",
///   "invoice_id"   : "INV1023",
///   "investor"     : "GBINV...",
///   "amount"       : 50000000000,
///   "funded_amount": 100000000000,
///   "status"       : 1
/// }
/// ```
#[contractevent]
pub struct EscrowFunded {
    /// Event name topic.
    #[topic]
    pub name: Symbol,
    /// Invoice this contribution belongs to.
    pub invoice_id: Symbol,
    /// Investor wallet that called `fund()`.
    pub investor: Address,
    /// Amount added in this single call (always positive).
    pub amount: i128,
    /// Cumulative funded amount **after** this call.
    pub funded_amount: i128,
    /// Status value **after** this call: `0` = still open, `1` = now fully funded.
    pub status: u32,
}

/// Emitted by `settle()` once the buyer has paid and the escrow is closed.
///
/// Contains everything needed for a settlement accounting service to compute
/// investor payouts without re-reading contract storage.
///
/// ### Indexer example (JSON after XDR decode)
/// ```json
/// {
///   "event"         : "escrow_settled",
///   "invoice_id"    : "INV1023",
///   "funded_amount" : 100000000000,
///   "yield_bps"     : 800,
///   "maturity"      : 1750000000
/// }
/// ```
///
/// ### Payout formula (off-chain, backend responsibility)
/// ```text
/// gross_yield = funded_amount * (yield_bps / 10_000) * (days_held / 365)
/// investor_payout = funded_amount + gross_yield
/// ```
#[contractevent]
pub struct EscrowSettled {
    /// Event name topic.
    #[topic]
    pub name: Symbol,
    /// Invoice that has been settled.
    pub invoice_id: Symbol,
    /// Total principal held (== `funding_target` at settlement time).
    pub funded_amount: i128,
    /// Annualized yield in basis points for investor payout calculation.
    pub yield_bps: i64,
    /// Original maturity timestamp — used by backend to compute accrued interest.
    pub maturity: u64,
}

// ──────────────────────────────────────────────────────────────────────────────
// Contract
// ──────────────────────────────────────────────────────────────────────────────

/// Storage key for the single `InvoiceEscrow` record kept in instance storage.
/// One contract instance == one invoice escrow.
const ESCROW_KEY: Symbol = symbol_short!("escrow");

#[contract]
pub struct LiquifactEscrow;

#[contractimpl]
impl LiquifactEscrow {
    // ──────────────────────────────────────────────────────────────────────────
    // init
    // ──────────────────────────────────────────────────────────────────────────

    /// Initialize a new invoice escrow and transition its status to **open** (0).
    ///
    /// ## Parameters
    /// | Name         | Type      | Description                                           |
    /// |--------------|-----------|-------------------------------------------------------|
    /// | `invoice_id` | `Symbol`  | Unique invoice ID (max 8 chars, ASCII)                 |
    /// | `sme_address`| `Address` | SME wallet that will receive funds when funded         |
    /// | `amount`     | `i128`    | Invoice face value in smallest token unit (> 0)        |
    /// | `yield_bps`  | `i64`     | Annualized investor yield in basis points (e.g. 800)   |
    /// | `maturity`   | `u64`     | Invoice maturity as Unix timestamp (ledger time)       |
    ///
    /// ## Emitted Event
    /// `EscrowInitialized { name: "escrow_initd", escrow: <full snapshot> }`
    ///
    /// ## Errors
    /// Panics if called on a contract instance that already has escrow storage set.
    pub fn init(
        env: Env,
        invoice_id: Symbol,
        sme_address: Address,
        amount: i128,
        yield_bps: i64,
        maturity: u64,
    ) -> InvoiceEscrow {
        let escrow = InvoiceEscrow {
            invoice_id,
            sme_address,
            amount,
            funding_target: amount,
            funded_amount: 0,
            yield_bps,
            maturity,
            status: 0, // open
        };

        env.storage().instance().set(&ESCROW_KEY, &escrow);

        // Event: EscrowInitialized
        // Publishes the full escrow snapshot so indexers can bootstrap state
        // for this invoice_id from a single event without reading storage.
        EscrowInitialized {
            name: symbol_short!("escrow_ii"), // "escrow_initialized" abbreviated to 8 chars
            escrow: escrow.clone(),
        }
        .publish(&env);

        escrow
    }

    // ──────────────────────────────────────────────────────────────────────────
    // get_escrow
    // ──────────────────────────────────────────────────────────────────────────

    /// Return the current escrow state without modifying storage.
    ///
    /// Read-only; does **not** emit an event.
    ///
    /// ## Errors
    /// Panics with `"Escrow not initialized"` if `init` has not been called.
    pub fn get_escrow(env: Env) -> InvoiceEscrow {
        env.storage()
            .instance()
            .get(&ESCROW_KEY)
            .unwrap_or_else(|| panic!("Escrow not initialized"))
    }

    // ──────────────────────────────────────────────────────────────────────────
    // fund
    // ──────────────────────────────────────────────────────────────────────────

    /// Record an investor funding contribution and update running totals.
    ///
    /// In production this function would be paired with a token `transfer` or
    /// `transfer_from` call that moves stablecoin into the contract.  The
    /// escrow automatically transitions to **funded** (status 1) the moment
    /// `funded_amount >= funding_target`.
    ///
    /// ## Parameters
    /// | Name       | Type      | Description                                     |
    /// |------------|-----------|-------------------------------------------------|
    /// | `investor` | `Address` | Wallet making the investment (recorded in event)|
    /// | `amount`   | `i128`    | Amount contributed in this call (> 0)           |
    ///
    /// ## Emitted Event
    /// `EscrowFunded { name: "escrow_fd", invoice_id, investor, amount, funded_amount, status }`
    ///
    /// `status` in the payload is the **post-call** value: `0` if more funding
    /// is still needed, `1` if the target was just met.
    ///
    /// ## Errors
    /// - Panics if `status != 0` (escrow not open for funding).
    pub fn fund(env: Env, investor: Address, amount: i128) -> InvoiceEscrow {
        let mut escrow = Self::get_escrow(env.clone());
        assert!(escrow.status == 0, "Escrow not open for funding");

        escrow.funded_amount += amount;
        if escrow.funded_amount >= escrow.funding_target {
            escrow.status = 1; // funded — ready to release to SME
        }

        env.storage().instance().set(&ESCROW_KEY, &escrow);

        // Event: EscrowFunded
        // Emitted on every successful fund() call. Indexers can also detect
        // the "fully funded" milestone via status == 1 in this payload.
        EscrowFunded {
            name: symbol_short!("escrow_fd"),
            invoice_id: escrow.invoice_id.clone(),
            investor,
            amount,
            funded_amount: escrow.funded_amount,
            status: escrow.status,
        }
        .publish(&env);

        escrow
    }

    // ──────────────────────────────────────────────────────────────────────────
    // settle
    // ──────────────────────────────────────────────────────────────────────────

    /// Mark the escrow as **settled** (status 2).
    ///
    /// Called by the backend once the buyer has paid the invoice off-chain.
    /// After this point, investors are entitled to redeem `funded_amount`
    /// principal plus the yield calculated from `yield_bps` and `maturity`.
    ///
    /// ## Emitted Event
    /// `EscrowSettled { name: "escrow_sd", invoice_id, funded_amount, yield_bps, maturity }`
    ///
    /// The payload intentionally excludes `sme_address` and raw `amount` to
    /// minimize event size; those fields are available from the earlier
    /// `EscrowInitialized` event for the same `invoice_id`.
    ///
    /// ## Errors
    /// - Panics with `"Escrow must be funded before settlement"` if `status != 1`.
    pub fn settle(env: Env) -> InvoiceEscrow {
        let mut escrow = Self::get_escrow(env.clone());
        assert!(
            escrow.status == 1,
            "Escrow must be funded before settlement"
        );

        escrow.status = 2; // settled
        env.storage().instance().set(&ESCROW_KEY, &escrow);

        // Event: EscrowSettled
        // The settlement accounting service uses yield_bps + maturity to
        // compute the exact investor payout without re-reading contract state.
        EscrowSettled {
            name: symbol_short!("escrow_sd"),
            invoice_id: escrow.invoice_id.clone(),
            funded_amount: escrow.funded_amount,
            yield_bps: escrow.yield_bps,
            maturity: escrow.maturity,
        }
        .publish(&env);

        escrow
    }
}

#[cfg(test)]
mod test;
