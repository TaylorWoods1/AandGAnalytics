//! Jira Cloud REST client for AandG Analytics.

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles_and_jira_crate_is_wired() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ag_jira");
    }
}
