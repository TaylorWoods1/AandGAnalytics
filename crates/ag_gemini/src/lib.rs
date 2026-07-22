//! Gemini client and context pack builder for AandG Analytics.

mod client;
mod context;
mod error;

pub use client::{
    suggested_prompts, GeminiAnswer, GeminiClient, DEFAULT_MODEL, SYSTEM_INSTRUCTION,
};
pub use context::{
    approx_token_count, build_context_pack, format_pack_for_prompt, ContextPack, IssueCite,
    MetricsFilter,
};
pub use error::GeminiError;
