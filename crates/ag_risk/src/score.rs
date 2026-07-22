//! Epic at-risk score (0 = safe … 100 = high risk).
//!
//! # Formula
//!
//! The score is a weighted sum of five drivers (weights sum to 100):
//!
//! | Driver | Weight | Component (0–1 before weighting) |
//! |--------|--------|-----------------------------------|
//! | Throughput pressure | 40 | `clamp(weeks_of_remaining_work / 12, 0, 1)` where weeks = remaining ÷ weekly throughput |
//! | Open-issue age (p50) | 15 | `clamp(age_days / 60, 0, 1)` |
//! | Blocked time | 15 | `clamp(blocked_days / 14, 0, 1)` |
//! | Recent scope growth | 15 | `clamp(recent_scope_growth, 0, 1)` (fraction) |
//! | Recent spillover rate | 15 | `clamp(recent_spillover_rate, 0, 1)` (fraction) |
//!
//! ```text
//! score = 40·throughput + 15·age + 15·blocked + 15·scope + 15·spillover
//! ```
//!
//! Drivers listed in the result are those contributing ≥ 5 points, highest first.
//! Zero / missing throughput with remaining work is treated as maximum pressure.

const W_THROUGHPUT: f64 = 40.0;
const W_AGE: f64 = 15.0;
const W_BLOCKED: f64 = 15.0;
const W_SCOPE: f64 = 15.0;
const W_SPILLOVER: f64 = 15.0;

const THROUGHPUT_WEEKS_CAP: f64 = 12.0;
const AGE_DAYS_CAP: f64 = 60.0;
const BLOCKED_DAYS_CAP: f64 = 14.0;
const DRIVER_MIN_POINTS: f64 = 5.0;

/// Inputs for epic at-risk scoring.
#[derive(Debug, Clone, PartialEq)]
pub struct EpicRiskInput {
    pub epic_key: String,
    pub remaining_issues: u32,
    pub remaining_points: Option<f64>,
    pub avg_weekly_throughput_issues: f64,
    pub avg_weekly_throughput_points: Option<f64>,
    pub blocked_secs_total: i64,
    pub open_issue_age_secs_p50: i64,
    pub recent_scope_growth: f64,
    pub recent_spillover_rate: f64,
}

/// Epic risk score and human-readable drivers.
#[derive(Debug, Clone, PartialEq)]
pub struct EpicRiskResult {
    pub epic_key: String,
    /// 0 = safe … 100 = high risk.
    pub score: f64,
    pub drivers: Vec<String>,
}

/// Score an epic's delivery risk from throughput, age, blocked time, scope, and spillover.
pub fn score_epic(input: &EpicRiskInput) -> EpicRiskResult {
    let throughput_c = throughput_component(input);
    let age_c = clamp01(input.open_issue_age_secs_p50 as f64 / 86_400.0 / AGE_DAYS_CAP);
    let blocked_c = clamp01(input.blocked_secs_total as f64 / 86_400.0 / BLOCKED_DAYS_CAP);
    let scope_c = clamp01(input.recent_scope_growth);
    let spillover_c = clamp01(input.recent_spillover_rate);

    let parts = [
        (
            W_THROUGHPUT * throughput_c,
            format!(
                "Throughput pressure ({:.0} pts): remaining work vs weekly completions",
                W_THROUGHPUT * throughput_c
            ),
        ),
        (
            W_AGE * age_c,
            format!(
                "Open issue age ({:.0} pts): median age of unfinished issues",
                W_AGE * age_c
            ),
        ),
        (
            W_BLOCKED * blocked_c,
            format!(
                "Blocked time ({:.0} pts): cumulative time in blocked/impediment statuses",
                W_BLOCKED * blocked_c
            ),
        ),
        (
            W_SCOPE * scope_c,
            format!(
                "Scope growth ({:.0} pts): recent net scope increases on the epic",
                W_SCOPE * scope_c
            ),
        ),
        (
            W_SPILLOVER * spillover_c,
            format!(
                "Spillover ({:.0} pts): recent sprint spillover rate",
                W_SPILLOVER * spillover_c
            ),
        ),
    ];

    let score = parts.iter().map(|(p, _)| *p).sum::<f64>().clamp(0.0, 100.0);

    let mut drivers: Vec<(f64, String)> = parts
        .into_iter()
        .filter(|(p, _)| *p >= DRIVER_MIN_POINTS)
        .collect();
    drivers.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let drivers = drivers.into_iter().map(|(_, d)| d).collect();

    EpicRiskResult {
        epic_key: input.epic_key.clone(),
        score,
        drivers,
    }
}

fn throughput_component(input: &EpicRiskInput) -> f64 {
    if input.remaining_issues == 0 {
        return 0.0;
    }
    let weekly = input.avg_weekly_throughput_issues;
    if weekly <= 0.0 {
        return 1.0;
    }
    let weeks = input.remaining_issues as f64 / weekly;
    clamp01(weeks / THROUGHPUT_WEEKS_CAP)
}

fn clamp01(v: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    v.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epic_risk_rises_with_low_throughput_and_scope_growth() {
        let low = score_epic(&EpicRiskInput {
            epic_key: "E-1".into(),
            remaining_issues: 40,
            remaining_points: None,
            avg_weekly_throughput_issues: 2.0,
            avg_weekly_throughput_points: None,
            blocked_secs_total: 0,
            open_issue_age_secs_p50: 0,
            recent_scope_growth: 0.5,
            recent_spillover_rate: 0.4,
        });
        let high_capacity = score_epic(&EpicRiskInput {
            epic_key: "E-2".into(),
            remaining_issues: 4,
            remaining_points: None,
            avg_weekly_throughput_issues: 10.0,
            avg_weekly_throughput_points: None,
            blocked_secs_total: 0,
            open_issue_age_secs_p50: 0,
            recent_scope_growth: 0.0,
            recent_spillover_rate: 0.0,
        });
        assert!(low.score > high_capacity.score);
    }
}
