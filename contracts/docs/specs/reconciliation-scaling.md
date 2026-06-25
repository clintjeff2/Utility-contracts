# Deposit → Token Reconciliation Scaling — Overflow Safety

Issue #5 — "Integer Scaling Protection Failure in Resource Deposit/Burnback
Reconciliation"

## Goal

Convert an off-chain resource deposit attestation into the number of on-chain
tokens to mint:

```
tokens_to_mint = floor(deposit_amount × TOKEN_SCALE_FACTOR / ASSET_PRECISION)
```

- `TOKEN_SCALE_FACTOR = 10¹⁸` (Soroban 18-decimal token standard)
- `ASSET_PRECISION ∈ [1, 10¹²]` (commodity micro-unit precision, configurable)

**Invariant:** `tokens_minted × ASSET_PRECISION ≈ deposit_amount × TOKEN_SCALE_FACTOR`
within < 1 base unit (floor rounding).

## The defect

A naive `u128` implementation computes `deposit_amount × 10¹⁸` first. For large
deposits (and the crafted `deposit = u128::MAX`, `ASSET_PRECISION = 1`) that
product exceeds `u128::MAX` and **wraps silently**, minting an amount wildly
divorced from the deposit — either a tiny fraction of, or vastly more than, the
backing resource.

## The implementation

`contracts/common/src/scaling.rs` (pure `#![no_std]`, no new dependencies):

- `reconcile_tokens(deposit_amount, asset_precision) -> Result<u128, ScaleError>`
  - validates `ASSET_PRECISION ∈ [1, 10¹²]` → `Err(InvalidPrecision)`;
  - computes the scaling with the exact 256-bit `mul_div_floor` from
    [`crate::weighted_rate`] — the `deposit × 10¹⁸` product is held in full
    256-bit precision and divided exactly, so it **never wraps**;
  - returns `Err(Overflow)` if the mathematically-correct token amount exceeds
    `u128::MAX` (instead of a silently wrapped value).
- `scale(amount, scale_factor, precision)` — the same, with a caller-supplied
  scale factor.
- `is_valid_precision`, `is_safe_deposit`, and `MAX_SAFE_DEPOSIT` helpers for
  callers that want the blueprint's conservative early-reject guard.

### Rounding

Floor is used deliberately: the contract must never mint **more** tokens than the
deposit backs. The error is strictly `< 1` base unit, satisfying the `|result −
exact| ≤ 1` requirement (in fact `< 1`).

### Why not a 512-bit `uint`-based `SafeScale`

The blueprint proposes a `(numerator, denominator)` struct over the `uint` crate
for 512-bit math. It is unnecessary: both operands are `u128`, so their product
is at most 256 bits, which `mul_div_floor` already handles exactly with no new
dependency and no allocation. The crate stays `no_std` and dependency-free.

## Tests (`contracts/common/src/scaling.rs`)

- simple conversions, zero deposit;
- precision bounds rejection (`0`, `10¹² + 1`, and the inclusive endpoints);
- the crafted `u128::MAX` overflow is **rejected, not wrapped**;
- `MAX_SAFE_DEPOSIT` boundary (no false overflow at the limit; overflow one past);
- large deposit with large precision still resolves;
- floor never over-mints, error < 1 base unit;
- 5000-iteration deterministic property sweep over
  `deposit ∈ [0, MAX_SAFE_DEPOSIT]`, `precision ∈ [1, 10¹²]`, asserting **exact**
  equality with a native `u128` reference.

Run: `cargo test --package utility-contracts-common`

## Wiring

There is no `reconcile_deposit` contract in the repository today (the issue's
`contracts/src/resource_tokenization/...` paths do not exist). The verified
primitive lives in `common` so any reconciliation entry point — e.g. a future
`reconcile_deposit` in `resource-token`, or the supply accounting in
`utility_contracts` once that crate compiles — can call
`utility_contracts_common::scaling::reconcile_tokens` instead of unchecked
`u128` multiplication.
