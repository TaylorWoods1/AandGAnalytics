//! Amazon Bedrock client and context pack builder for Jira Analytics.

mod client;
mod context;
mod error;

pub use client::{
    suggested_prompts, AiAnswer, BedrockClient, DEFAULT_MODEL, SYSTEM_INSTRUCTION,
};
pub use context::{
    approx_token_count, build_context_pack, format_pack_for_prompt, ContextPack, IssueCite,
    MetricsFilter,
};
pub use error::BedrockError;
