//! Memory forgetting — time-based decay of memory salience (SoulLedger).
//!
//! Memories lose salience the longer they go unaccessed, modelled as an
//! exponential half-life decay over `importance`. Memories that fall below a
//! floor are candidates for soft-deletion ("forgetting"), while sufficiently
//! important memories are protected and never forgotten regardless of age.
//!
//! This module is **pure and side-effect free** so the policy can be unit
//! tested without a database or clock. The DB sweep that consumes it lives in
//! the chat post-stream path: memory retrieval bumps `access_count` /
//! `last_accessed_at`, then a cooldown-gated background sweep soft-deletes
//! candidates returned here. A reference sketch:
//!
//! ```ignore
//! // For each row (id, importance, last_accessed_at, created_at):
//! let reference = last_accessed_at.unwrap_or(created_at);
//! let days = days_since(reference, now);
//! if let Verdict::Forget { .. } = evaluate(importance, days, &ForgettingPolicy::default()) {
//!     // UPDATE memories SET deleted_at = NOW() WHERE id = $1 AND user_id = $2
//! }
//! ```

use chrono::{DateTime, Utc};

// ── Policy ────────────────────────────────────────────────────────

/// Tunable forgetting parameters. `Default` encodes the MVP policy.
#[derive(Debug, Clone, Copy)]
pub struct ForgettingPolicy {
    /// Days for a memory's effective importance to halve when unaccessed.
    pub half_life_days: f64,
    /// Effective importance below which a memory is forgotten.
    pub forget_floor: f32,
    /// Base importance at or above which a memory is never forgotten
    /// (core identity / explicitly-important facts).
    pub protect_importance: f32,
}

impl Default for ForgettingPolicy {
    fn default() -> Self {
        Self {
            half_life_days: 30.0,
            forget_floor: 0.15,
            protect_importance: 0.9,
        }
    }
}

// ── Verdict ───────────────────────────────────────────────────────

/// Outcome of evaluating a single memory against the policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verdict {
    /// Keep the memory. `effective` is its current decayed importance.
    Keep { effective: f32 },
    /// Forget (soft-delete) the memory. `effective` is what it decayed to.
    Forget { effective: f32 },
}

impl Verdict {
    pub fn effective(&self) -> f32 {
        match self {
            Verdict::Keep { effective } | Verdict::Forget { effective } => *effective,
        }
    }

    pub fn should_forget(&self) -> bool {
        matches!(self, Verdict::Forget { .. })
    }
}

// ── Pure functions ────────────────────────────────────────────────

/// Whole and fractional days from `reference` to `now`. Never negative:
/// a `reference` in the future (clock skew) is treated as "just accessed".
pub fn days_since(reference: DateTime<Utc>, now: DateTime<Utc>) -> f64 {
    let seconds = (now - reference).num_seconds();
    if seconds <= 0 {
        0.0
    } else {
        seconds as f64 / 86_400.0
    }
}

/// Exponential half-life decay multiplier in `[0, 1]`.
///
/// Returns `1.0` (no decay) for non-positive `half_life_days`, and clamps a
/// negative `days` to `0`. `decay_factor(h, h) == 0.5`.
pub fn decay_factor(days: f64, half_life_days: f64) -> f64 {
    if half_life_days <= 0.0 {
        return 1.0;
    }
    let days = days.max(0.0);
    0.5_f64.powf(days / half_life_days)
}

/// Current decayed importance of a memory, clamped to `[0, 1]`.
pub fn effective_importance(base_importance: f32, days: f64, half_life_days: f64) -> f32 {
    let base = base_importance.clamp(0.0, 1.0) as f64;
    (base * decay_factor(days, half_life_days)).clamp(0.0, 1.0) as f32
}

/// Whether a memory's *base* importance protects it from ever being forgotten.
pub fn is_protected(base_importance: f32, policy: &ForgettingPolicy) -> bool {
    base_importance >= policy.protect_importance
}

/// Evaluate one memory against the policy.
pub fn evaluate(base_importance: f32, days: f64, policy: &ForgettingPolicy) -> Verdict {
    let effective = effective_importance(base_importance, days, policy.half_life_days);

    if is_protected(base_importance, policy) {
        return Verdict::Keep { effective };
    }
    if effective < policy.forget_floor {
        Verdict::Forget { effective }
    } else {
        Verdict::Keep { effective }
    }
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, mo: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, 0, 0, 0).unwrap()
    }

    // ── days_since ────────────────────────────────────────────────

    #[test]
    fn days_since_counts_whole_days() {
        let d = days_since(at(2026, 1, 1), at(2026, 1, 11));
        assert!((d - 10.0).abs() < 1e-9);
    }

    #[test]
    fn days_since_counts_fractional_days() {
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        assert!((days_since(start, end) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn days_since_future_reference_is_zero() {
        // Clock skew: reference after now → treat as just accessed.
        assert_eq!(days_since(at(2026, 2, 1), at(2026, 1, 1)), 0.0);
    }

    #[test]
    fn days_since_same_instant_is_zero() {
        assert_eq!(days_since(at(2026, 1, 1), at(2026, 1, 1)), 0.0);
    }

    // ── decay_factor ──────────────────────────────────────────────

    #[test]
    fn decay_factor_is_one_at_zero_days() {
        assert!((decay_factor(0.0, 30.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn decay_factor_halves_at_one_half_life() {
        assert!((decay_factor(30.0, 30.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn decay_factor_quarters_at_two_half_lives() {
        assert!((decay_factor(60.0, 30.0) - 0.25).abs() < 1e-9);
    }

    #[test]
    fn decay_factor_non_positive_half_life_means_no_decay() {
        assert_eq!(decay_factor(1000.0, 0.0), 1.0);
        assert_eq!(decay_factor(1000.0, -5.0), 1.0);
    }

    #[test]
    fn decay_factor_clamps_negative_days() {
        assert!((decay_factor(-10.0, 30.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn decay_factor_is_monotonically_decreasing() {
        let a = decay_factor(10.0, 30.0);
        let b = decay_factor(20.0, 30.0);
        let c = decay_factor(40.0, 30.0);
        assert!(a > b && b > c);
    }

    // ── effective_importance ──────────────────────────────────────

    #[test]
    fn effective_importance_fresh_equals_base() {
        assert!((effective_importance(0.8, 0.0, 30.0) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn effective_importance_halves_after_one_half_life() {
        assert!((effective_importance(0.8, 30.0, 30.0) - 0.4).abs() < 1e-6);
    }

    #[test]
    fn effective_importance_clamps_base_into_range() {
        // Out-of-range base values are clamped before decay.
        assert!(effective_importance(2.0, 0.0, 30.0) <= 1.0);
        assert!(effective_importance(-1.0, 0.0, 30.0) >= 0.0);
    }

    // ── is_protected ──────────────────────────────────────────────

    #[test]
    fn is_protected_above_threshold() {
        let p = ForgettingPolicy::default();
        assert!(is_protected(0.9, &p));
        assert!(is_protected(0.95, &p));
    }

    #[test]
    fn is_protected_below_threshold() {
        let p = ForgettingPolicy::default();
        assert!(!is_protected(0.89, &p));
    }

    // ── evaluate ──────────────────────────────────────────────────

    #[test]
    fn evaluate_keeps_fresh_memory() {
        let p = ForgettingPolicy::default();
        let v = evaluate(0.6, 0.0, &p);
        assert!(!v.should_forget());
    }

    #[test]
    fn evaluate_forgets_faded_low_importance_memory() {
        let p = ForgettingPolicy::default();
        // base 0.5, ~90 days (3 half-lives) → 0.0625 < 0.15 floor
        let v = evaluate(0.5, 90.0, &p);
        assert!(v.should_forget());
        assert!(v.effective() < p.forget_floor);
    }

    #[test]
    fn evaluate_protects_high_importance_even_when_ancient() {
        let p = ForgettingPolicy::default();
        // Protected by base importance regardless of decayed value.
        let v = evaluate(0.95, 3650.0, &p);
        assert!(!v.should_forget());
    }

    #[test]
    fn evaluate_keeps_when_just_above_floor() {
        let p = ForgettingPolicy::default();
        // Choose days so effective stays >= floor: base 0.5 at 1 half-life = 0.25 > 0.15
        let v = evaluate(0.5, 30.0, &p);
        assert!(!v.should_forget());
    }

    #[test]
    fn evaluate_boundary_exactly_at_floor_is_kept() {
        // effective == floor must NOT forget (strict `<`).
        let p = ForgettingPolicy {
            half_life_days: 30.0,
            forget_floor: 0.5,
            protect_importance: 0.9,
        };
        // base 1.0 at exactly one half-life → 0.5 == floor → keep
        let v = evaluate(1.0, 30.0, &p);
        assert!(!v.should_forget(), "effective == floor should be kept");
    }

    #[test]
    fn evaluate_verdict_carries_effective_value() {
        let p = ForgettingPolicy::default();
        let v = evaluate(0.8, 30.0, &p);
        assert!((v.effective() - 0.4).abs() < 1e-6);
    }
}
