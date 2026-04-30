mod app;
mod ui;

use std::thread;
use std::time::Duration;

use app::{
    InMemoryControlClient, LiveWallSettingsApp, NamedPipeControlClient, sample_status_snapshot,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--ui") => {
            let client = NamedPipeControlClient::default();
            let app = LiveWallSettingsApp::bootstrap(client)?;
            ui::run_ui(app)?;
        }
        Some("--pipe") => {
            let client = NamedPipeControlClient::default();
            let app = LiveWallSettingsApp::bootstrap(client)?;
            println!("{}", serde_json::to_string_pretty(app.snapshot())?);
        }
        Some("--pipe-watch") => {
            let interval_ms = args
                .next()
                .map(|value| value.parse::<u64>())
                .transpose()?
                .unwrap_or(1_000);
            run_pipe_watch(Duration::from_millis(interval_ms))?;
        }
        Some("--print-status") | None => {
            let client = InMemoryControlClient::new(sample_status_snapshot());
            let app = LiveWallSettingsApp::bootstrap(client)?;
            println!("{}", serde_json::to_string_pretty(app.snapshot())?);
        }
        Some(other) => {
            return Err(format!("unsupported argument `{other}`").into());
        }
    }

    Ok(())
}

fn run_pipe_watch(interval: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let client = NamedPipeControlClient::default();
    let mut app = LiveWallSettingsApp::bootstrap(client)?;

    loop {
        println!("{}", serde_json::to_string_pretty(app.snapshot())?);
        thread::sleep(interval);
        app.refresh()?;
    }
}
