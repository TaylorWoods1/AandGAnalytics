//! Flow and delivery analytics metrics for AandG Analytics.

mod bottleneck;
mod changelog;
mod error;
mod events;
mod flow;
mod rebuild;
mod sprint;
mod throughput;

pub use bottleneck::bottleneck_by_status;
pub use changelog::{transitions_from_changelog, StatusTransition};
pub use error::AnalyticsError;
pub use events::{detect_handoffs, detect_reopens, detect_scope_changes, FieldChange, ScopeChangeStats};
pub use flow::{
    cycle_and_lead, default_status_category, resolve_status_category, time_in_status,
    CycleLeadTimes, StatusFlowCategory, TimeInStatus,
};
pub use rebuild::{
    rebuild_all_derived, rebuild_event_derived, rebuild_flow_derived, rebuild_sprint_derived,
    rebuild_throughput_derived,
};
pub use sprint::{compute_sprint_metrics, SprintMetrics};
pub use throughput::daily_throughput;

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles_and_analytics_crate_is_wired() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ag_analytics");
    }
}
