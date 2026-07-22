//! Finish-by probability from remaining work and weekly throughput.
//!
//! # Model
//!
//! Treat weekly throughput as i.i.d. draws with mean `μ` and stddev `σ`. Over
//! `W` weeks until the target, expected completed work is `μ·W` with stddev
//! `σ·√W` (independent weeks). The finish-by probability is the normal
//! survival probability that total throughput meets remaining work:
//!
//! ```text
//! P(finish) = 1 − Φ((R − μW) / (σ√W))
//! ```
//!
//! where `R` is remaining issues. Probability is clamped to `[0, 1]`. Edge
//! cases: remaining ≤ 0 → 1; `W` ≤ 0 with remaining > 0 → 0; `σ` = 0 →
//! deterministic compare of `μW` vs `R`.

/// Inputs for a finish-by probability estimate.
#[derive(Debug, Clone, PartialEq)]
pub struct FinishByInput {
    pub remaining_work_issues: f64,
    pub weekly_throughput_issues: f64,
    pub weeks_until_target: f64,
    pub throughput_stddev: f64,
}

/// Finish-by result with explicit modeling assumptions for the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct FinishByResult {
    pub probability: f64,
    pub assumptions: Vec<String>,
}

/// Estimate P(finish by target) via a normal approximation on cumulative throughput.
pub fn finish_by_probability(input: &FinishByInput) -> FinishByResult {
    let mut assumptions = vec![
        "Weekly throughput is modeled as i.i.d. normal draws.".into(),
        "Weeks are independent; cumulative stddev scales with √W.".into(),
        "No scope change after the forecast is made.".into(),
        "Finish means remaining issue count reaches zero.".into(),
    ];

    let remaining = input.remaining_work_issues;
    let mu = input.weekly_throughput_issues.max(0.0);
    let weeks = input.weeks_until_target;
    let sigma = input.throughput_stddev.max(0.0);

    if remaining <= 0.0 {
        assumptions.push("No remaining work; probability is 1.".into());
        return FinishByResult {
            probability: 1.0,
            assumptions,
        };
    }

    if weeks <= 0.0 {
        assumptions.push("Target date is not in the future; probability is 0.".into());
        return FinishByResult {
            probability: 0.0,
            assumptions,
        };
    }

    let mean = mu * weeks;
    assumptions.push(format!(
        "Expected completed issues by target: {mean:.2} (= {mu:.2}/week × {weeks:.2} weeks)."
    ));

    let probability = if sigma == 0.0 {
        assumptions.push("Throughput stddev is 0; using deterministic comparison.".into());
        if mean >= remaining {
            1.0
        } else {
            0.0
        }
    } else {
        let std_total = sigma * weeks.sqrt();
        let z = (remaining - mean) / std_total;
        // P(X >= remaining) = 1 - Φ((remaining - mean) / std) = Φ((mean - remaining) / std)
        let p = 1.0 - normal_cdf(z);
        assumptions.push(format!(
            "Normal CDF with μ={mean:.2}, σ={std_total:.2}, threshold={remaining:.2}."
        ));
        p.clamp(0.0, 1.0)
    };

    FinishByResult {
        probability: probability.clamp(0.0, 1.0),
        assumptions,
    }
}

/// Standard normal CDF via Abramowitz & Stegun 7.1.26 (max |error| < 7.5e-8).
fn normal_cdf(z: f64) -> f64 {
    if z.is_nan() {
        return 0.5;
    }
    if z > 8.0 {
        return 1.0;
    }
    if z < -8.0 {
        return 0.0;
    }

    let sign = if z < 0.0 { -1.0 } else { 1.0 };
    let x = z.abs() / std::f64::consts::SQRT_2;

    // erf approximation
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let erf = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();

    0.5 * (1.0 + sign * erf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_by_probability_is_high_when_throughput_covers_remaining() {
        let r = finish_by_probability(&FinishByInput {
            remaining_work_issues: 10.0,
            weekly_throughput_issues: 10.0,
            weeks_until_target: 2.0,
            throughput_stddev: 1.0,
        });
        assert!(r.probability > 0.8);
        assert!(!r.assumptions.is_empty());
    }
}
