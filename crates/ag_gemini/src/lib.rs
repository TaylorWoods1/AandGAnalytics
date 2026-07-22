//! Gemini client and context pack builder for AandG Analytics.

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles_and_gemini_crate_is_wired() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ag_gemini");
    }
}
