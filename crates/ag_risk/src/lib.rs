//! Epic at-risk and finish-by probability for AandG Analytics.

mod error;
mod forecast;
mod rebuild;
mod score;

pub use error::RiskError;
pub use forecast::{finish_by_probability, FinishByInput, FinishByResult};
pub use rebuild::rebuild_epic_risk;
pub use score::{score_epic, EpicRiskInput, EpicRiskResult};

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles_and_risk_crate_is_wired() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ag_risk");
    }
}
