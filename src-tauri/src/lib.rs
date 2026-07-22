//! Tauri application shell for AandG Analytics.
//!
//! Command handlers and scheduler wiring land in a later task.

pub fn app_name() -> &'static str {
    "AandG Analytics"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_name_is_wired() {
        assert_eq!(app_name(), "AandG Analytics");
    }
}
