# LiquiFact Escrow Contract

Soroban smart contracts for **LiquiFact** on Stellar. This repository contains the `escrow` crate: a single-instance invoice escrow with funding, optional SME collateral **records**, compliance **legal hold**, SME withdrawal, settlement, and per-investor accounting.

## Contract API (summary)

| Method | Purpose |
|--------|---------|
| `init` | Create escrow (admin auth). Sets `funding_target = amount`. |
| `get_escrow` / `get_version` / `get_legal_hold` | Read state. |
| `get_contribution` | Per-investor funded principal. |
| `update_funding_target` | Admin, open state only; target ≥ `funded_amount`. |
| `fund` | Investor auth; blocked while legal hold is active. |
| `withdraw` | SME auth; funded → withdrawn; blocked under legal hold. |
| `settle` | SME auth; funded → settled (maturity gate if set); blocked under legal hold. |
| `claim_investor_payout` | Investor auth; after settle; blocked under legal hold. |
| `record_sme_collateral_commitment` | SME auth; **record-only** pledge (asset + amount + timestamp). |
| `get_sme_collateral_commitment` / `is_investor_claimed` | Reads. |
| `set_legal_hold` / `clear_legal_hold` | Admin governance (hold blocks risk-bearing transitions). |
| `update_maturity` | Admin, open state only. |
| `transfer_admin` | Admin rotation. |
| `migrate` | Version guardrails (see upgrade policy below). |

### Optional SME collateral (record-only)

`record_sme_collateral_commitment` stores a [`SmeCollateralCommitment`](escrow/src/lib.rs) under `DataKey::SmeCollateralPledge`. It does **not** lock tokens on-chain or trigger liquidation. Indexers should treat it as a disclosure field for future enforcement hooks; **no false liquidation** is possible from this field alone because no asset movement or status transition depends on it.

### Legal / compliance hold

When `DataKey::LegalHold` is true, the contract rejects new `fund`, `settle`, SME `withdraw`, and `claim_investor_payout`. Only the stored **admin** may set or clear the hold. **Emergency policy:** there is no separate break-glass entrypoint; recovery is via governed `admin` (multisig / DAO). Document operational playbooks off-chain so holds cannot strand funds without governance.

### Storage keys (`DataKey`)

Public enum in [`escrow/src/lib.rs`](escrow/src/lib.rs): `Escrow`, `Version`, `InvestorContribution(Address)`, `LegalHold`, `SmeCollateralPledge`, `InvestorClaimed(Address)`. New optional keys should keep **additive** names and avoid reusing or repurposing existing variants.

---

## Storage-only upgrade policy (additive fields)

**Compatible without redeploy** when you only:

- Add **new** `DataKey` variants and/or new `#[contracttype]` structs stored under **new** keys.
- Read new keys with `.get(...).unwrap_or(default)` so missing keys behave as “unset” on old deployments.

**Requires new deployment or explicit migration** when you:

- Change layout or meaning of an existing stored type (e.g. new required field on `InvoiceEscrow` without a migration that rewrites `DataKey::Escrow`).
- Rename or change the XDR shape of an existing `DataKey` variant used in production.

**Compatibility test plan (short):**

1. Deploy version _N_; exercise `init`, `fund`, `settle`.
2. Deploy version _N+1_ with only new optional keys; repeat flows; assert old instances still readable.
3. If `InvoiceEscrow` changes, add a migration test or document mandatory redeploy.

`migrate` today validates `from_version` against stored `DataKey::Version` and errors if no path is implemented.

### `DataKey` naming convention

Use PascalCase variant names matching persisted role (`LegalHold`, `SmeCollateralPledge`). Per-address maps use wrapper variants: `InvestorContribution(Address)`, `InvestorClaimed(Address)`.

---

## Release runbook: build, deploy, verify

**Who may deploy production:** only addresses and keys owned by LiquiFact governance (multisig / custody). Treat contract admin and deployer secrets as **highly sensitive**.

### Environment variables (example)

| Variable | Purpose |
|----------|---------|
| `STELLAR_NETWORK` | e.g. `TESTNET` / `PUBLIC` / custom Horizon passphrase |
| `SOROBAN_RPC_URL` | Soroban RPC endpoint |
| `SOURCE_SECRET` | Funding / deployer Stellar secret key (S ...) |
| `LIQUIFACT_ADMIN_ADDRESS` | Initial admin intended to control holds and funding target |

Exact CLI flags change between Soroban releases; always cross-check [Stellar Soroban docs](https://developers.stellar.org/docs/tools/soroban-cli/stellar-cli) for your installed `stellar` / `soroban` CLI version.

### Build WASM

```bash
rustup target add wasm32v1-none
cargo build --target wasm32v1-none --release
# Artifact (typical):
# target/wasm32v1-none/release/liquifact_escrow.wasm
```

### Deploy (example flow)

```bash
stellar contract deploy \
  --wasm target/wasm32v1-none/release/liquifact_escrow.wasm \
  --source-account "$SOURCE_SECRET" \
  --network "$STELLAR_NETWORK" \
  --rpc-url "$SOROBAN_RPC_URL"
# Record emitted contract id as LIQUIFACT_ESCROW_CONTRACT_ID
```

Initialize on-chain with `init` via `stellar contract invoke` (pass `admin`, `invoice_id`, `sme_address`, amounts, `yield_bps`, `maturity` per your product).

### Verify artifact hash

```bash
shasum -a 256 target/wasm32v1-none/release/liquifact_escrow.wasm
```

Store the digest in release notes and inject the same WASM into verification tooling (block explorer, internal registry). After deployment, confirm the **on-chain contract code hash** matches the audited artifact for that release tag.

### Backend / config registration

- Persist `LIQUIFACT_ESCROW_CONTRACT_ID` (and network passphrase) in the backend’s secure config.
- Rollback: **cannot** undeploy a contract; rollback is *forward-only*: deploy a new contract id, point new traffic to it, and sunset the old id. Document state replication needs if invoices were already bound to the old id.

---

## Local development and CI

| Step | Command |
|------|---------|
| Format | `cargo fmt --all -- --check` |
| Build | `cargo build` |
| Test | `cargo test` |
| Coverage (≥ 95% lines in CI) | `cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only` |

Install coverage tools:

```bash
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
```

---

## Security notes

- **Auth:** state-changing entrypoints use `require_auth()` for the appropriate role (admin, SME, investor).
- **Legal hold:** is governance-controlled; misuse risk is mitigated by using a multisig `admin` and operational policy.
- **Collateral record:** is not proof of encumbrance until a future version explicitly enforces token transfers.
- **Overflow:** `fund` uses `checked_add` on `funded_amount`.

### Contract type clone/derive safety

- `DataKey` keeps `Clone` because key wrappers are reused for storage get/set paths.
- `InvoiceEscrow` and `SmeCollateralCommitment` intentionally do **not** derive `Clone`; this prevents accidental full-state duplication in hot paths.
- `InvoiceEscrow` and `SmeCollateralCommitment` derive `PartialEq` for deterministic state assertions in tests and `Debug` for failure diagnostics.
- `init` publishes `EscrowInitialized` from stored state instead of cloning the in-memory escrow snapshot, reducing avoidable copy overhead.

---

## Contributing

1. Branch from `main`.
2. Run `cargo fmt`, `cargo test`, and the coverage command above before pushing.
3. Keep README and rustdoc aligned with `escrow/src/lib.rs` behavior.
