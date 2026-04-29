mod bootstrap;

use bootstrap::{LiveWallService, ServiceOptions, run_desktop_smoke_test};
use livewall_control::{Command, CommandEnvelope};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--desktop-smoke-test") => {
            run_desktop_smoke_test()?;
        }
        Some("--ipc-smoke-test") => {
            let mut service = LiveWallService::bootstrap(ServiceOptions::default(), Vec::new())?;
            let response = service.handle_envelope(CommandEnvelope::new(1, Command::GetStatus));
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        Some("--once") | None => {
            let service = LiveWallService::bootstrap(ServiceOptions::default(), Vec::new())?;
            println!("{}", serde_json::to_string_pretty(&service.snapshot())?);
        }
        Some(other) => {
            return Err(format!("unsupported argument `{other}`").into());
        }
    }

    Ok(())
}
