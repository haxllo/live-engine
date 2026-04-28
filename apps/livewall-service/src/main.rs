mod bootstrap;

use bootstrap::{LiveWallService, ServiceOptions, run_desktop_smoke_test};

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
