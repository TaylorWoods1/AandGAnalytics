#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(feature = "desktop")]
    {
        aandg_analytics_tauri::run();
        return;
    }

    #[cfg(not(feature = "desktop"))]
    {
        // Library/command crate builds without native webview deps.
        // Production desktop: `cargo run -p aandg-analytics-tauri --features desktop`
        println!(
            "{} (command library; enable `--features desktop` for the Tauri app)",
            aandg_analytics_tauri::app_name()
        );
    }
}
