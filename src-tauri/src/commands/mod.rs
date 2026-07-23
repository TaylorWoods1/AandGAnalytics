//! Tauri IPC command handlers (library-testable `*_inner` + optional desktop wrappers).

pub mod ai;
pub mod maintenance;
pub mod metrics;
pub mod setup;
pub mod sync;

pub use ai::{
    ask_ai_inner, get_suggested_prompts_inner, preview_context_pack_inner, AiAnswerDto,
    ContextPackDto, SuggestedPromptsDto,
};
pub use maintenance::{
    clear_raw_issue_data, full_resync_inner, rebuild_derived, rebuild_derived_inner,
    reset_sync_checkpoints,
};
pub use metrics::{
    get_epic_risk_inner, get_finish_by_inner, get_flow_metrics_inner, get_performance_metrics_inner,
    get_sprint_metrics_inner, list_issues_inner, BottleneckDto, EpicRiskDto, FinishByResultDto,
    FlowMetricsDto, IssuePageDto, IssueRowDto, MetricsFilter, Page, PerformanceMetricsDto,
    PersonMonthDto, PersonVelocityDto, ProjectMonthDto, ProjectPerfDto, SprintMetricsDto,
    ThroughputPointDto,
};
pub use setup::{
    get_setup_info_inner, get_story_points_mapping_inner, reset_setup_inner, save_setup_inner,
    set_story_points_mapping_inner, validate_setup_inner, BedrockCredentialsDto, FieldCandidateDto,
    JiraCredentialsDto, SetupInfoDto, SetupStatus, StoryPointsMappingDto,
};
pub use sync::{get_sync_progress_inner, start_full_sync_inner, start_incremental_sync_inner};

#[cfg(feature = "desktop")]
pub use ai::{ask_ai, get_suggested_prompts, preview_context_pack};
#[cfg(feature = "desktop")]
pub use maintenance::{full_resync, rebuild_derived_cmd};
#[cfg(feature = "desktop")]
pub use metrics::{
    get_epic_risk, get_finish_by, get_flow_metrics, get_performance_metrics, get_sprint_metrics,
    list_issues,
};
#[cfg(feature = "desktop")]
pub use setup::{
    get_setup_info, get_story_points_mapping, reset_setup, save_setup, set_story_points_mapping,
    validate_setup,
};
#[cfg(feature = "desktop")]
pub use sync::{get_sync_progress, start_full_sync, start_incremental_sync};
