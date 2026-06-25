//! Overflow-safe deposit → token reconciliation scaling.
//!
//! Issue #5 — reconciling on-chain token supply with off-chain resource deposit
//! attestations computes
//!
//! ```text
//!     tokens_to_mint = deposit_amount × TOKEN_SCALE_FACTOR / ASSET_PRECISION
//! ```
//!
//! with `TOKEN_SCALE_FACTOR = 10¹⁸` (Soroban token decimals) and a configurable
//! `ASSET_PRECISION` (commodity micro-unit precision). A naive `u128`
//! implementation overflows the intermediate product `deposit × 10¹⁸` and wraps
//! **silently** — minting wildly wrong amounts (the issue's `u128::MAX` craft).
//!
//! This module computes the scaling with the exact 256-bit `mul_div` from
//! [`crate::weighted_rate`]: the intermediate product is held in full 256-bit
//! precision and divided exactly, so there is **no silent overflow** — an input
//! whose true token result would exceed `u128` is reported as
//! [`ScaleError::Overflow`], never a wrapped value. `ASSET_PRECISION` is bounds-
//! validated, and rejection is returned as `Result` rather than a panic so the
//! caller can reconcile gracefully.

use crate::weighted_rate::mul_div_floor;

/// Soroban token scale factor: 10¹⁸ (18 decimals).
pub const TOKEN_SCALE_FACTOR: u128 = 1_000_000_000_000_000_000;

/// Minimum valid `ASSET_PRECISION`.
pub const MIN_ASSET_PRECISION: u128 = 1;

/// Maximum valid `ASSET_PRECISION`: 10¹².
pub const MAX_ASSET_PRECISION: u128 = 1_000_000_000_000;

/// Largest deposit whose product with [`TOKEN_SCALE_FACTOR`] still fits `u128`.
/// Deposits up to this bound are guaranteed representable for any precision ≥ 1;
/// larger deposits are still handled correctly (256-bit intermediate) and only
/// rejected if the *final* token amount would exceed `u128`.
pub const MAX_SAFE_DEPOSIT: u128 = u128::MAX / TOKEN_SCALE_FACTOR;

/// Errors from reconciliation scaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleError {
    /// `ASSET_PRECISION` is outside `[MIN_ASSET_PRECISION, MAX_ASSET_PRECISION]`.
    InvalidPrecision,
    /// The resulting token amount would exceed `u128::MAX`.
    Overflow,
}

/// Whether `precision` is within the configured `[1, 10¹²]` bounds.
pub fn is_valid_precision(precision: u128) -> bool {
    precision >= MIN_ASSET_PRECISION && precision <= MAX_ASSET_PRECISION
}

/// Whether `deposit_amount × TOKEN_SCALE_FACTOR` fits in `u128` (i.e. the
/// conservative "safe range" guard from the resolution blueprint). The main
/// [`reconcile_tokens`] does not require this — it handles larger deposits via
/// 256-bit arithmetic — but callers wanting an early reject can use it.
pub fn is_safe_deposit(deposit_amount: u128) -> bool {
    deposit_amount <= MAX_SAFE_DEPOSIT
}

/// Reconcile a resource deposit into the number of tokens to mint:
///
/// ```text
///     floor(deposit_amount × TOKEN_SCALE_FACTOR / asset_precision)
/// ```
///
/// Computed with exact 256-bit intermediate precision (no silent overflow).
/// Floor rounding is used deliberately: the contract must never mint **more**
/// tokens than the deposit backs (rounding error is strictly < 1 base unit).
///
/// # Errors
/// * [`ScaleError::InvalidPrecision`] if `asset_precision ∉ [1, 10¹²]`.
/// * [`ScaleError::Overflow`] if the (mathematically valid) token amount exceeds
///   `u128::MAX`.
pub fn reconcile_tokens(deposit_amount: u128, asset_precision: u128) -> Result<u128, ScaleError> {
    if !is_valid_precision(asset_precision) {
        return Err(ScaleError::InvalidPrecision);
    }
    // mul_div_floor holds deposit × 10¹⁸ in 256 bits and divides exactly;
    // `None` means the true quotient does not fit u128.
    mul_div_floor(deposit_amount, TOKEN_SCALE_FACTOR, asset_precision).ok_or(ScaleError::Overflow)
}

/// General overflow-safe scaling: `floor(amount × scale_factor / precision)`.
/// Same guarantees as [`reconcile_tokens`] but with a caller-supplied scale.
///
/// # Errors
/// * [`ScaleError::InvalidPrecision`] if `precision == 0`.
/// * [`ScaleError::Overflow`] if the result exceeds `u128::MAX`.
pub fn scale(amount: u128, scale_factor: u128, precision: u128) -> Result<u128, ScaleError> {
    if precision == 0 {
        return Err(ScaleError::InvalidPrecision);
    }
    mul_div_floor(amount, scale_factor, precision).ok_or(ScaleError::Overflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconciles_simple_amounts() {
        // 1 micro-unit at precision 1 -> 10^18 tokens.
        assert_eq!(reconcile_tokens(1, 1), Ok(TOKEN_SCALE_FACTOR));
        // 1 commodity unit (10^6 micro-units) at precision 10^6 -> 1 token (10^18).
        assert_eq!(reconcile_tokens(1_000_000, 1_000_000), Ok(TOKEN_SCALE_FACTOR));
        // zero deposit -> zero tokens.
        assert_eq!(reconcile_tokens(0, 1_000), Ok(0));
    }

    #[test]
    fn rejects_invalid_precision() {
        assert_eq!(reconcile_tokens(100, 0), Err(ScaleError::InvalidPrecision));
        assert_eq!(
            reconcile_tokens(100, MAX_ASSET_PRECISION + 1),
            Err(ScaleError::InvalidPrecision)
        );
        assert!(reconcile_tokens(100, MAX_ASSET_PRECISION).is_ok());
        assert!(reconcile_tokens(100, MIN_ASSET_PRECISION).is_ok());
    }

    #[test]
    fn crafted_overflow_input_is_rejected_not_wrapped() {
        // The issue's craft: deposit == u128::MAX, precision == 1.
        // Naive u128 would wrap silently; we must report Overflow.
        assert_eq!(reconcile_tokens(u128::MAX, 1), Err(ScaleError::Overflow));
        // Even the max precision cannot bring u128::MAX * 10^18 back into range
        // (10^18 / 10^12 = 10^6 > 1).
        assert_eq!(
            reconcile_tokens(u128::MAX, MAX_ASSET_PRECISION),
            Err(ScaleError::Overflow)
        );
    }

    #[test]
    fn boundary_of_safe_deposit() {
        // deposit == MAX_SAFE_DEPOSIT, precision 1: product is ~u128::MAX and
        // must NOT be a false overflow (256-bit intermediate handles it).
        let expected = MAX_SAFE_DEPOSIT * TOKEN_SCALE_FACTOR; // fits u128 by definition
        assert_eq!(reconcile_tokens(MAX_SAFE_DEPOSIT, 1), Ok(expected));

        // One past the safe deposit at precision 1 overflows the u128 result.
        assert_eq!(
            reconcile_tokens(MAX_SAFE_DEPOSIT + 1, 1),
            Err(ScaleError::Overflow)
        );
    }

    #[test]
    fn large_deposit_with_large_precision_fits() {
        // deposit just above MAX_SAFE_DEPOSIT, but a precision that scales the
        // result back under u128 — the 256-bit path returns the exact value
        // where naive u128 would have overflowed the intermediate product.
        let deposit = MAX_SAFE_DEPOSIT + 1_000;
        // precision 10^6 -> result ~= deposit * 10^12, still < u128::MAX.
        let result = reconcile_tokens(deposit, 1_000_000).unwrap();
        // Cross-check against the same exact 256-bit primitive.
        assert_eq!(
            result,
            crate::weighted_rate::mul_div_floor(deposit, TOKEN_SCALE_FACTOR, 1_000_000).unwrap()
        );
    }

    #[test]
    fn floor_rounding_never_over_mints() {
        // deposit*scale not divisible by precision -> floor, error < 1 unit.
        // 7 * 10^18 / 3 = 2.333...e18 -> floor.
        let r = reconcile_tokens(7, 3).unwrap();
        let exact_lower = (7u128 * TOKEN_SCALE_FACTOR) / 3;
        assert_eq!(r, exact_lower);
        // tokens * precision <= deposit * scale (never over-mints).
        assert!(r * 3 <= 7 * TOKEN_SCALE_FACTOR);
        // and within 1 base unit of exact.
        assert!(7 * TOKEN_SCALE_FACTOR - r * 3 < 3);
    }

    #[test]
    fn scale_helper_matches_and_guards_zero() {
        assert_eq!(scale(0, 1, 1), Ok(0));
        assert_eq!(scale(10, 5, 2), Ok(25));
        assert_eq!(scale(10, 5, 0), Err(ScaleError::InvalidPrecision));
        assert_eq!(scale(u128::MAX, u128::MAX, 1), Err(ScaleError::Overflow));
    }

    #[test]
    fn property_exact_within_safe_numerator_domain() {
        // Deterministic sweep. Restrict deposit so deposit*SCALE fits u128, so a
        // native u128 reference is valid; assert EXACT equality (error 0, far
        // tighter than the "<= 1 base unit" target).
        let mut seed: u64 = 0xDEAD_BEEF_CAFE_F00D;
        let mut next = || {
            seed ^= seed >> 12;
            seed ^= seed << 25;
            seed ^= seed >> 27;
            seed.wrapping_mul(0x2545F4914F6CDD1D)
        };
        let mut next_u128 = || ((next() as u128) << 64) | (next() as u128);

        for _ in 0..5000 {
            let deposit = next_u128() % (MAX_SAFE_DEPOSIT + 1); // deposit*SCALE fits u128
            let precision = (next_u128() % MAX_ASSET_PRECISION) + 1; // [1, 10^12]

            let reference = (deposit * TOKEN_SCALE_FACTOR) / precision; // exact in u128
            assert_eq!(reconcile_tokens(deposit, precision), Ok(reference));
        }
    }
}
