mod app;

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
        Some("--pipe") => {
            let client = NamedPipeControlClient::default();
            let app = LiveWallSettingsApp::bootstrap(client)?;
            println!("{}", serde_json::to_string_pretty(app.snapshot())?);
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
