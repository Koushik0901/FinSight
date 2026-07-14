//! Window-vs-baseline decomposition: rank the drivers of a period's spend
//! against "your normal" and tag each on two axes — mechanism (where the
//! delta came from) and persistence (will it repeat). The LLM never computes
//! these; it narrates them.

use crate::spending::{Mechanism, Persistence};

/// Ratio at/above which a change in ticket size or frequency is "up"/"down".
const CHANGE_RATIO: f64 = 1.3;

/// Per-merchant recent vs baseline monthly figures → a mechanism.
pub(crate) fn classify_mechanism(
    recent_monthly: i64,
    base_monthly: i64,
    recent_txns_pm: f64,
    base_txns_pm: f64,
) -> Mechanism {
    if base_monthly == 0 && recent_monthly > 0 {
        return Mechanism::New;
    }
    if recent_monthly == 0 && base_monthly > 0 {
        return Mechanism::Stopped;
    }
    let recent_ticket = if recent_txns_pm > 0.0 { recent_monthly as f64 / recent_txns_pm } else { 0.0 };
    let base_ticket = if base_txns_pm > 0.0 { base_monthly as f64 / base_txns_pm } else { 0.0 };
    let freq = if base_txns_pm > 0.0 { recent_txns_pm / base_txns_pm } else { f64::INFINITY };
    let price = if base_ticket > 0.0 { recent_ticket / base_ticket } else { f64::INFINITY };
    let freq_up = freq >= CHANGE_RATIO;
    let price_up = price >= CHANGE_RATIO;
    let freq_dn = freq <= 1.0 / CHANGE_RATIO;
    let price_dn = price <= 1.0 / CHANGE_RATIO;
    match (freq_up, price_up, freq_dn, price_dn) {
        (true, true, _, _) => Mechanism::Mixed,
        (true, _, _, _) => Mechanism::FrequencyUp,
        (_, true, _, _) => Mechanism::PriceUp,
        (_, _, true, _) => Mechanism::FrequencyDown,
        (_, _, _, true) => Mechanism::PriceDown,
        _ => Mechanism::Flat,
    }
}

/// Persistence from cheap structural signals (Phase 1). A later plan refines
/// this with recurring.rs cadence + user annotations.
pub(crate) fn classify_persistence(
    mechanism: Mechanism,
    active_months: i64,
    total_txns: i64,
    target_txns: i64,
) -> Persistence {
    if active_months >= 4 {
        return Persistence::Recurring;
    }
    if matches!(mechanism, Mechanism::New) && target_txns >= 2 {
        return Persistence::Emerging; // new and already repeating within the window
    }
    if total_txns <= 2 {
        return Persistence::OneOff;
    }
    Persistence::Uncertain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mechanism_distinguishes_new_price_frequency() {
        assert_eq!(classify_mechanism(5000, 0, 1.0, 0.0), Mechanism::New);
        assert_eq!(classify_mechanism(0, 5000, 0.0, 1.0), Mechanism::Stopped);
        // Same ~1 txn/mo, ticket doubled → PriceUp.
        assert_eq!(classify_mechanism(20000, 10000, 1.0, 1.0), Mechanism::PriceUp);
        // Same $10 ticket (7000/7 == 11000/11 == 1000), more visits → FrequencyUp.
        assert_eq!(classify_mechanism(11000, 7000, 11.0, 7.0), Mechanism::FrequencyUp);
        // Steady.
        assert_eq!(classify_mechanism(10000, 9800, 2.0, 2.0), Mechanism::Flat);
    }

    #[test]
    fn persistence_reads_structure() {
        assert_eq!(classify_persistence(Mechanism::PriceUp, 8, 20, 3), Persistence::Recurring);
        assert_eq!(classify_persistence(Mechanism::New, 1, 3, 3), Persistence::Emerging);
        assert_eq!(classify_persistence(Mechanism::New, 1, 1, 1), Persistence::OneOff);
    }
}
