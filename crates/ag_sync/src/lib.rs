//! Sync orchestration between Jira and local storage for AandG Analytics.

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles_and_sync_crate_is_wired() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ag_sync");
    }
}
