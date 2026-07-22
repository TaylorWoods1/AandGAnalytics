//! Flow and delivery analytics metrics for AandG Analytics.

mod changelog;
mod error;
mod flow;
mod rebuild;

pub use changelog::{transitions_from_changelog, StatusTransition};
pub use error::AnalyticsError;
pub use flow::{
    cycle_and_lead, default_status_category, resolve_status_category, time_in_status,
    CycleLeadTimes, StatusFlowCategory, TimeInStatus,
};
pub use rebuild::rebuild_flow_derived;

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles_and_analytics_crate_is_wired() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ag_analytics");
    }
}
