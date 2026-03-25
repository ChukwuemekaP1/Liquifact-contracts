# LiquiFact Escrow Contract – Threat Model & Security Notes

Soroban smart contracts for **LiquiFact** — the global invoice liquidity network on Stellar.
This repo contains the **escrow** contract that holds investor funds for tokenized invoices until settlement.

---

## Threat Model

### 1. Unauthorized Access

**Risk:**
- Anyone can call `fund` or `settle`

**Impact:**
- Malicious settlement
- Fake funding events

**Mitigation (Current):**
- None (mock auth used in tests)

**Recommended Controls:**
- Require auth:
  - `fund`: investor must authorize
  - `settle`: only trusted role (e.g. admin/oracle)

---

### 2. Arithmetic Risks (Overflow / Underflow)

**Risk:**
- `funded_amount += amount` may overflow `i128`

---

### 3. Replay / Double Execution

```bash
git clone <this-repo-url>
cd liquifact-contracts
cargo build
cargo test
```

---

### 5. Invalid Input / Economic Attacks

**Risks:**
- Negative funding
- Zero funding
- Invalid maturity

| Command                | Description                                                |
|------------------------|------------------------------------------------------------|
| `cargo build`          | Build all contracts                                        |
| `cargo test`           | Run unit tests and property-based tests (using `proptest`) |
| `cargo fmt`            | Format code                                                |
| `cargo fmt -- --check` | Check formatting (used in CI)                              |

---

### 6. Time-based Attacks

```text
liquifact-contracts/
├── Cargo.toml              # Workspace definition
├── docs/
│   └── EVENT_SCHEMA.md    # Indexer-friendly event schema reference
├── escrow/
│   ├── Cargo.toml          # Escrow contract crate
│   └── src/
│       ├── lib.rs       # LiquiFact escrow contract (init, fund, settle, migrate)
│       └── test.rs      # Unit tests
├── docs/
│   ├── openapi.yaml     # OpenAPI 3.1 specification
│   ├── package.json     # Test runner deps (AJV, js-yaml)
│   └── tests/
│       └── openapi.test.js  # Schema conformance tests (51 cases)
└── .github/workflows/
    └── ci.yml              # CI: fmt, build, test
```

Records an investor contribution. Transitions to `status = 1` when
`funded_amount >= funding_target`.

> **Production note:** Must be called atomically with a SEP-41 token `transfer`
> from `investor` to the contract address. This version records accounting only.

**Parameters**

| Parameter   | Constraints                                  |
|-------------|----------------------------------------------|
| `_investor` | Investor's Stellar address (for audit trail) |
| `amount`    | > 0 recommended; partial funding is allowed  |

**Returns** — Updated `InvoiceEscrow`.

**Failure conditions**

| Condition                 | Behaviour                               |
|---------------------------|-----------------------------------------|
| `status != 0`             | Panics: `"Escrow not open for funding"` |
| `init` not called         | Panics: `"Escrow not initialized"`      |
| `funded_amount` overflows | Rust panics (debug) / wraps (release)   |

//! # LiquiFact Escrow Contract
//!
//! Holds investor funds for an invoice until settlement.
//!
//! ### Settlement Sequence
//! 1. **Initialization**: Admin creates the escrow with `init`.
//! 2. **Funding**: Investors contribute funds via `fund` until `funding_target` is met (status 0 -> 1).
//! 3. **Settlement**: SME calls `settle` once the buyer has repaid the invoice (status 1 -> 2).
//! 4. **Claim**: Investors call `claim` to withdraw their principal plus accrued yield (status 2).

## State transitions

- **init** — Create an invoice escrow. Requires `admin` authorization.
- **get_escrow** — Read current escrow state.
- **fund** — Record investor funding. Requires `investor` authorization. Status becomes "funded" (1) when target is met.
- **settle** — Mark escrow as settled. Requires `sme_address` authorization. Status becomes "settled" (2).
- **claim** — Investors redeem principal + yield. Requires `investor` authorization.
- **migrate** — Upgrade storage schema.

## Payout formula
```text
investor_payout = principal + (principal * yield_bps / 10_000)
```

## Security & Authorization

The contract enforces strict authorization via `require_auth()`:
- `init`: Only the designated `admin` can initialize.
- `fund`: Only the `investor` can fund on their own behalf.
- `settle`: Only the `sme_address` (beneficiary) can trigger settlement.
- `claim`: Only the `investor` can claim their own payout.
- `update_maturity`: Only the `admin` can update maturity (only in Open state).

---


## Funding Constraints
- **Minimum Funding:** All funding amounts must be strictly greater than zero ($> 0$). 
- **Initialization:** Escrow creation will fail if the target amount is not positive.
- **Integer Safety:** Uses `checked_add` to prevent overflow during funded amount accounting.

---

## Security Assumptions

- Soroban runtime guarantees:
- Deterministic execution
- Storage integrity
- Token transfers handled externally
- Off-chain systems validate invoice authenticity

---

---

## Invariants

- `funded_amount <= funding_target` (soft enforced)
- `status transitions`: 0 → 1 → 2
- Cannot settle before funded
| Step | Command | Fails if… |
|------|---------|-----------|
| Format | `cargo fmt --all -- --check` | any file is not formatted |
| Build | `cargo build` | compilation error |
| Tests | `cargo test` | any test fails |
| Coverage | `cargo llvm-cov --features testutils --fail-under-lines 95` | line coverage < 95 % |

### Coverage gate

The pipeline uses [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) (installed via `taiki-e/install-action`) to measure line coverage and hard-fail the job when it drops below **95 %**.

To run the coverage check locally:

```bash
# Install once
cargo install cargo-llvm-cov

# Run (requires llvm-tools-preview component)
rustup component add llvm-tools-preview
cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only
```

Keep formatting, tests, and coverage passing before opening a PR.

---

## Contributing

1. **Fork** the repo and clone your fork.
2. **Create a branch** from `main`: `git checkout -b feature/your-feature` or `fix/your-fix`.
3. **Setup**: ensure Rust stable is installed; run `cargo build` and `cargo test`.
4. **Make changes**:
   - Follow existing patterns in `escrow/src/lib.rs`.
   - Add or update tests in `escrow/src/test.rs`.
   - Format with `cargo fmt`.
5. **Verify locally**:
   - `cargo fmt --all -- --check`
   - `cargo build`
   - `cargo test --features testutils`
6. **Commit** with clear messages (e.g. `feat(escrow): X`, `test(escrow): Y`).
7. **Push** to your fork and open a **Pull Request** to `main`.
8. Wait for CI and address review feedback.

We welcome new contracts (e.g. settlement, tokenization helpers), tests, and docs that align with LiquiFact's invoice financing flow.

---

## Future Improvements

- Multi-escrow support
- Role-based access control
- Token integration
- Event emission
- Formal verification
