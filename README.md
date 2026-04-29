/*!
# LiveWall

LiveWall is a Windows 11 live wallpaper app built in Rust.

The v1 product goal is a lightweight consumer runtime that supports:

- hardware-decoded video wallpapers
- precompiled shader scene wallpapers
- per-monitor assignment
- fullscreen auto-pause
- battery-aware throttling

## Bootstrap Status

This repository is intentionally bootstrapped with a temporary root crate so
`cargo metadata --format-version 1` works before the real workspace members
exist.

The planned members are:

- `apps/livewall-service`
- `apps/livewall-settings`
- `crates/livewall-control`
- `crates/livewall-pack`
- `crates/livewall-desktop`
- `crates/livewall-render`
- `crates/livewall-video`
- `crates/livewall-engine`

Once those manifests land, the root manifest should switch from the bootstrap
crate to the concrete workspace member list from the implementation plan.

## Workspace Commands

- `cargo metadata --format-version 1`
- `cargo xtest`
- `cargo xlint`
- `cargo xfmt`
- `cargo run -p livewall-service -- --once`
- `cargo run -p livewall-service -- --serve`
- `cargo run -p livewall-service -- --serve-once`
- `cargo run -p livewall-service -- --ipc-smoke-test`
- `cargo run -p livewall-service -- --scene-smoke-test`
- `cargo run -p livewall-service -- --video-smoke-test`
- `cargo test -p livewall-settings`
- `cargo run -p livewall-settings -- --print-status`
- `cargo run -p livewall-settings -- --pipe`
- `powershell -ExecutionPolicy Bypass -File scripts/smoke-test.ps1`
- `powershell -ExecutionPolicy Bypass -File scripts/package-release.ps1`

## Runtime Flow

- Start the service loop: `cargo run -p livewall-service -- --serve`
- Query over named pipe: `cargo run -p livewall-settings -- --pipe`

The service loop uses blocking named-pipe I/O to avoid busy waiting and keep
idle CPU usage low.
*/
