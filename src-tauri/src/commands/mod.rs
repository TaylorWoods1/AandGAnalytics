//! Tauri IPC command handlers (library-testable `*_inner` + optional desktop wrappers).

pub mod metrics;
pub mod setup;
pub mod sync;

pub use metrics::{
    get_epic_risk_inner, get_finish_by_inner, get_flow_metrics_inner, get_sprint_metrics_inner,
    list_issues_inner, BottleneckDto, EpicRiskDto, FinishByResultDto, FlowMetricsDto, IssuePageDto,
    IssueRowDto, MetricsFilter, Page, SprintMetricsDto, ThroughputPointDto,
};
pub use setup::{
    save_setup_inner, validate_setup_inner, GeminiCredentialsDto, JiraCredentialsDto, SetupStatus,
};
pub use sync::{get_sync_progress_inner, start_full_sync_inner, start_incremental_sync_inner};

#[cfg(feature = "desktop")]
pub use metrics::{
    get_epic_risk, get_finish_by, get_flow_metrics, get_sprint_metrics, list_issues,
};
#[cfg(feature = "desktop")]
pub use setup::{save_setup, validate_setup};
#[cfg(feature = "desktop")]
pub use sync::{get_sync_progress, start_full_sync, start_incremental_sync};
