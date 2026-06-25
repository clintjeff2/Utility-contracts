/// Default protocol fee rate in basis points (1% = 100 bps)
#[allow(dead_code)]
pub const FEE_RATE_BPS: u32 = 100;

/// Minimum fee rate in basis points (0.01%)
#[allow(dead_code)]
pub const MIN_FEE_RATE_BPS: u32 = 1;

/// Maximum fee rate in basis points (10%)
pub const MAX_FEE_RATE_BPS: u32 = 1000;

/// Maximum settlement amount with 7-decimal precision
#[allow(dead_code)]
pub const MAX_SETTLEMENT: i128 = 1_000_000_000_000_000_000; // 1e18

/// Denominator for basis points calculations
pub const BPS_DENOMINATOR: u32 = 10000;

/// Maximum allowed slippage in basis points (100 bps = 1%)
/// Range: [1, 500] (0.01% to 5%)
pub const MAX_SLIPPAGE_BPS: u32 = 100;

/// Maximum settlement volume per call (1M tokens with 7 decimals)
#[allow(dead_code)]
pub const MAX_VOLUME: i128 = 10_000_000_000_000; // 1_000_000 * 1e7

/// Maximum oracle rate with 7 decimals
#[allow(dead_code)]
pub const MAX_RATE: i128 = 10_000_000_000_000; // 1_000_000 * 1e7

/// Denominator for fixed-point operations (7 decimal places)
pub const DECIMAL_DENOMINATOR: i128 = 10_000_000; // 1e7

// --- Oracle staleness protection (issue #7) ------------------------------

/// Maximum age (in epoch-seconds) of an oracle price before it is considered
/// stale. Compared against `env.ledger().timestamp()`.
///
/// Must lie within `[300, 3600]` (5 minutes .. 1 hour); enforced at compile
/// time below.
pub const MAX_ORACLE_AGE: u64 = 600; // 10 minutes

/// Lower bound for [`MAX_ORACLE_AGE`] (5 minutes).
pub const MIN_ORACLE_AGE_BOUND: u64 = 300;
/// Upper bound for [`MAX_ORACLE_AGE`] (1 hour).
pub const MAX_ORACLE_AGE_BOUND: u64 = 3600;

// Compile-time guarantee that the configured staleness window is in range.
const _: () = assert!(
    MAX_ORACLE_AGE >= MIN_ORACLE_AGE_BOUND && MAX_ORACLE_AGE <= MAX_ORACLE_AGE_BOUND,
    "MAX_ORACLE_AGE must be within [300, 3600] seconds"
);

/// Conservative fallback rate used when the oracle price is stale.
/// `50_000_000` in 7-decimal fixed point (= 5.0), per issue #7 step 1.
pub const FALLBACK_RATE: i128 = 50_000_000;

/// Maximum allowable relative deviation of the computed rate from the true rate,
/// `0.001 * 1e7` in 7-decimal fixed point (i.e. 0.1%). Used as the tolerance
/// bound documented by the staleness invariant.
#[allow(dead_code)]
pub const MAX_STALENESS_TOLERANCE: i128 = 10_000;
