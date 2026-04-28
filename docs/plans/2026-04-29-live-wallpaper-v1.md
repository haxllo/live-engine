# Live Wallpaper V1 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Windows 11 consumer app in Rust that plays video and shader live wallpapers with low idle overhead, multi-monitor support, and automatic pause/throttle policies.

**Architecture:** A background service owns desktop attachment, rendering, decode, and policy decisions. A separate settings app talks to the service over named pipes and never hosts wallpaper content. Video wallpapers use Media Foundation with D3D11 interop; shader wallpapers use a small D3D11 render path with precompiled shaders. HTML content is out of scope for v1.

**Tech Stack:** Rust workspace, `windows` crate, Direct3D 11, DXGI, Media Foundation, named pipes, `slint` for settings UI, `tracing`, `serde`, `cargo test`, `cargo clippy`, `cargo fmt`

---

## Product Boundaries

- Target OS: Windows 11 only
- Wallpaper types: `video`, `scene`
- Explicit non-goals for v1: HTML wallpapers, marketplace/workshop, audio-reactive scenes, online sync, editor
- Consumer UX bar: one-click install, tray controls, preview thumbnails, startup on boot, pause on fullscreen app
- Performance bar: idle CPU near zero for paused wallpapers, hardware decode for video, one D3D11 device shared across monitors, source-fps playback on AC power unless policy reduces it

## Proposed Repository Layout

```text
Cargo.toml
rust-toolchain.toml
.cargo/config.toml
README.md
docs/architecture.md
docs/plans/2026-04-29-live-wallpaper-v1.md
apps/livewall-service/Cargo.toml
apps/livewall-service/src/main.rs
apps/livewall-service/src/bootstrap.rs
apps/livewall-settings/Cargo.toml
apps/livewall-settings/src/main.rs
apps/livewall-settings/src/app.rs
crates/livewall-control/Cargo.toml
crates/livewall-control/src/lib.rs
crates/livewall-control/tests/ipc_contract.rs
crates/livewall-pack/Cargo.toml
crates/livewall-pack/src/lib.rs
crates/livewall-pack/src/manifest.rs
crates/livewall-pack/src/install.rs
crates/livewall-pack/tests/manifest_tests.rs
crates/livewall-desktop/Cargo.toml
crates/livewall-desktop/src/lib.rs
crates/livewall-desktop/src/workerw.rs
crates/livewall-desktop/src/monitors.rs
crates/livewall-desktop/tests/desktop_model.rs
crates/livewall-render/Cargo.toml
crates/livewall-render/src/lib.rs
crates/livewall-render/src/device.rs
crates/livewall-render/src/scene.rs
crates/livewall-render/tests/scene_config.rs
crates/livewall-video/Cargo.toml
crates/livewall-video/src/lib.rs
crates/livewall-video/src/player.rs
crates/livewall-video/src/clock.rs
crates/livewall-video/tests/clock_tests.rs
crates/livewall-engine/Cargo.toml
crates/livewall-engine/src/lib.rs
crates/livewall-engine/src/policy.rs
crates/livewall-engine/src/runtime.rs
crates/livewall-engine/tests/policy_tests.rs
wallpapers/samples/coast-video/manifest.json
wallpapers/samples/coast-video/preview.jpg
wallpapers/samples/coast-video/video.mp4
wallpapers/samples/aurora-scene/manifest.json
wallpapers/samples/aurora-scene/preview.jpg
wallpapers/samples/aurora-scene/shaders/aurora.ps.cso
wallpapers/samples/aurora-scene/shaders/fullscreen.vs.cso
scripts/package-release.ps1
scripts/smoke-test.ps1
```

## Core Decisions

- Use one long-running process, `livewall-service`, for all wallpaper windows and playback. This avoids duplicate GPU devices and lets one scheduler enforce pause/throttle policies.
- Keep `livewall-settings` separate so the consumer UI can crash or update without dropping wallpapers.
- Use `Media Foundation` plus `IMFDXGIDeviceManager` for hardware decode. Avoid browser runtimes and software FFmpeg decode in v1.
- Define scene wallpapers as packages with precompiled shaders. Do not ship runtime shader compilation in v1.
- Install wallpaper packages into `%LOCALAPPDATA%\\LiveWall\\wallpapers\\<wallpaper-id>`. Treat imported `.livewall` files as zip archives that get extracted there.

## V1 Feature Slice

- Install and remove packaged wallpapers
- Render one wallpaper per monitor
- Support per-monitor assignment and a global "apply to all monitors" action
- Pause all wallpapers when a fullscreen game or video app is active
- Optional battery saver policy for laptops
- Tray actions: pause/resume, next wallpaper, open settings, quit
- Settings window: library, monitor assignment, performance mode, startup toggle

### Task 1: Bootstrap the Rust workspace

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.cargo/config.toml`
- Create: `README.md`
- Create: `docs/architecture.md`

**Step 1: Create the workspace manifest**

Define workspace members for `apps/livewall-service`, `apps/livewall-settings`, and all `crates/livewall-*` packages.

**Step 2: Pin the toolchain and workspace lints**

Use stable Rust, enable `clippy` and `rustfmt`, and centralize common lint settings.

**Step 3: Add Windows-focused cargo aliases**

Add aliases for `cargo xtest`, `cargo xlint`, and `cargo xfmt` in `.cargo/config.toml`.

**Step 4: Write the initial architecture note**

Document the service/settings split, supported wallpaper types, and explicit v1 non-goals in `docs/architecture.md`.

**Step 5: Verify the empty workspace shape**

Run: `cargo metadata --format-version 1`
Expected: the workspace resolves without missing-member errors.

**Step 6: Commit**

```bash
git add Cargo.toml rust-toolchain.toml .cargo/config.toml README.md docs/architecture.md
git commit -m "chore: bootstrap livewall workspace"
```

### Task 2: Define shared control and IPC contracts

**Files:**
- Create: `crates/livewall-control/Cargo.toml`
- Create: `crates/livewall-control/src/lib.rs`
- Create: `crates/livewall-control/tests/ipc_contract.rs`

**Step 1: Write the failing IPC contract tests**

Create tests for command serialization and event round-tripping:

```rust
#[test]
fn command_round_trip() {
    let command = Command::SetWallpaper {
        monitor_id: "DISPLAY1".into(),
        wallpaper_id: "coast-video".into(),
    };
    let json = serde_json::to_string(&command).unwrap();
    let decoded: Command = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, command);
}
```

**Step 2: Run the contract test and verify it fails**

Run: `cargo test -p livewall-control ipc_contract -v`
Expected: FAIL because `Command` and `Event` do not exist yet.

**Step 3: Implement the DTOs and protocol version**

Define:

```rust
pub enum Command {
    GetStatus,
    SetWallpaper { monitor_id: String, wallpaper_id: String },
    PauseAll,
    ResumeAll,
    SetPerformanceMode { mode: PerformanceMode },
}
```

Also define `Event`, `StatusSnapshot`, and a `PROTOCOL_VERSION`.

**Step 4: Re-run the tests**

Run: `cargo test -p livewall-control -v`
Expected: PASS.

**Step 5: Add a small compatibility note**

Document that the service rejects mismatched protocol versions and returns a typed error.

**Step 6: Commit**

```bash
git add crates/livewall-control
git commit -m "feat: add shared IPC contracts"
```

### Task 3: Implement wallpaper package manifests and installation

**Files:**
- Create: `crates/livewall-pack/Cargo.toml`
- Create: `crates/livewall-pack/src/lib.rs`
- Create: `crates/livewall-pack/src/manifest.rs`
- Create: `crates/livewall-pack/src/install.rs`
- Create: `crates/livewall-pack/tests/manifest_tests.rs`

**Step 1: Write failing manifest parser tests**

Cover valid `video` and `scene` manifests, missing fields, and unsupported versions.

```rust
#[test]
fn parses_video_manifest() {
    let manifest = r#"{
      "id":"coast-video",
      "version":1,
      "title":"Coast",
      "kind":"video",
      "entry":"video.mp4",
      "preview":"preview.jpg"
    }"#;
    let parsed = parse_manifest(manifest).unwrap();
    assert_eq!(parsed.id, "coast-video");
}
```

**Step 2: Run the manifest tests**

Run: `cargo test -p livewall-pack manifest_tests -v`
Expected: FAIL because the parser does not exist.

**Step 3: Implement `Manifest`, validation, and install paths**

Support:

```rust
pub enum WallpaperKind {
    Video { entry: PathBuf, loop_mode: LoopMode },
    Scene { vertex_shader: PathBuf, pixel_shader: PathBuf, config: Option<PathBuf> },
}
```

Add validation for file presence, package version, and duplicate wallpaper ids.

**Step 4: Implement package install/extract**

Treat `.livewall` as a zip archive, extract to `%LOCALAPPDATA%\\LiveWall\\wallpapers\\<id>`, and return an `InstalledWallpaper`.

**Step 5: Re-run package tests**

Run: `cargo test -p livewall-pack -v`
Expected: PASS.

**Step 6: Commit**

```bash
git add crates/livewall-pack
git commit -m "feat: add wallpaper package parser and installer"
```

### Task 4: Build desktop attachment and monitor discovery

**Files:**
- Create: `crates/livewall-desktop/Cargo.toml`
- Create: `crates/livewall-desktop/src/lib.rs`
- Create: `crates/livewall-desktop/src/workerw.rs`
- Create: `crates/livewall-desktop/src/monitors.rs`
- Create: `crates/livewall-desktop/tests/desktop_model.rs`

**Step 1: Write failing tests for the monitor model**

Test pure data transformations first: monitor ordering, primary selection, and placement normalization.

**Step 2: Run the tests**

Run: `cargo test -p livewall-desktop desktop_model -v`
Expected: FAIL because `MonitorInfo` and helpers are missing.

**Step 3: Implement monitor enumeration wrappers**

Create a safe wrapper around `EnumDisplayMonitors` that returns:

```rust
pub struct MonitorInfo {
    pub id: String,
    pub is_primary: bool,
    pub bounds_px: RectI32,
    pub work_area_px: RectI32,
    pub dpi: u32,
}
```

**Step 4: Implement WorkerW attachment**

Create helpers that locate the desktop host window, create a borderless child window per monitor, and keep the handles in a registry owned by the service.

**Step 5: Add a smoke-only verification path**

Add a small `attach_smoke_test()` helper that creates the wallpaper windows, paints a solid color for three seconds, then tears them down.

Run: `cargo test -p livewall-desktop -v`
Expected: unit tests PASS.

Run manually on Windows: `cargo run -p livewall-service -- --desktop-smoke-test`
Expected: a solid wallpaper window appears behind icons and disappears cleanly.

**Step 6: Commit**

```bash
git add crates/livewall-desktop
git commit -m "feat: add desktop host and monitor discovery"
```

### Task 5: Build the D3D11 renderer and scene runtime

**Files:**
- Create: `crates/livewall-render/Cargo.toml`
- Create: `crates/livewall-render/src/lib.rs`
- Create: `crates/livewall-render/src/device.rs`
- Create: `crates/livewall-render/src/scene.rs`
- Create: `crates/livewall-render/tests/scene_config.rs`

**Step 1: Write failing scene configuration tests**

Test scene package loading, shader path resolution, and default uniforms.

**Step 2: Run the scene tests**

Run: `cargo test -p livewall-render scene_config -v`
Expected: FAIL because the scene loader is missing.

**Step 3: Implement shared device creation**

Create one D3D11 device, immediate context, and DXGI factory that the service shares across monitors.

**Step 4: Implement a minimal scene renderer**

Render a fullscreen triangle with a precompiled vertex shader and pixel shader. Pass time, resolution, and optional wallpaper config uniforms each frame.

**Step 5: Add a scene smoke path**

Run: `cargo test -p livewall-render -v`
Expected: unit tests PASS.

Run manually on Windows: `cargo run -p livewall-service -- --scene-smoke-test wallpapers/samples/aurora-scene`
Expected: the aurora scene animates on the desktop at target fps.

**Step 6: Commit**

```bash
git add crates/livewall-render
git commit -m "feat: add d3d11 renderer and scene runtime"
```

### Task 6: Add Media Foundation video playback

**Files:**
- Create: `crates/livewall-video/Cargo.toml`
- Create: `crates/livewall-video/src/lib.rs`
- Create: `crates/livewall-video/src/player.rs`
- Create: `crates/livewall-video/src/clock.rs`
- Create: `crates/livewall-video/tests/clock_tests.rs`

**Step 1: Write failing clock and loop tests**

Test loop boundaries, pause/resume timing, and frame scheduling drift.

**Step 2: Run the video tests**

Run: `cargo test -p livewall-video clock_tests -v`
Expected: FAIL because the playback clock is missing.

**Step 3: Implement the playback clock**

Model `playing`, `paused`, `seeking`, and `looping` states independent of Media Foundation so policy logic can be unit-tested.

**Step 4: Implement hardware-decoded frame delivery**

Build a `VideoPlayer` around `IMFSourceReader` and `IMFDXGIDeviceManager` that outputs decoded frames into D3D11 textures.

**Step 5: Add a video smoke path**

Run: `cargo test -p livewall-video -v`
Expected: unit tests PASS.

Run manually on Windows: `cargo run -p livewall-service -- --video-smoke-test wallpapers/samples/coast-video/video.mp4`
Expected: the sample video loops without frame tearing and CPU stays low.

**Step 6: Commit**

```bash
git add crates/livewall-video
git commit -m "feat: add media foundation video playback"
```

### Task 7: Implement the runtime scheduler and performance policy

**Files:**
- Create: `crates/livewall-engine/Cargo.toml`
- Create: `crates/livewall-engine/src/lib.rs`
- Create: `crates/livewall-engine/src/policy.rs`
- Create: `crates/livewall-engine/src/runtime.rs`
- Create: `crates/livewall-engine/tests/policy_tests.rs`

**Step 1: Write failing policy tests**

Cover fullscreen pause, battery saver throttling, display sleep, and monitor hotplug updates.

```rust
#[test]
fn fullscreen_app_forces_pause() {
    let state = PolicyState::default().with_fullscreen_app(true);
    let decision = decide_frame_policy(&state, PerformanceMode::Balanced);
    assert_eq!(decision.playback_state, PlaybackState::Paused);
}
```

**Step 2: Run the policy tests**

Run: `cargo test -p livewall-engine policy_tests -v`
Expected: FAIL because the policy engine is missing.

**Step 3: Implement policy evaluation**

Define:

```rust
pub struct FrameDecision {
    pub playback_state: PlaybackState,
    pub target_fps: u32,
    pub decode_allowed: bool,
}
```

Policies:
- `Quality`: source fps on AC, pause on fullscreen
- `Balanced`: cap scenes at 30 fps when idle, pause on fullscreen
- `BatterySaver`: 24-30 fps cap, pause on battery threshold, reduce decode work

**Step 4: Implement the runtime coordinator**

The coordinator owns monitor assignments, instantiates scene/video players, reacts to commands from IPC, and publishes `StatusSnapshot` updates.

**Step 5: Re-run the tests**

Run: `cargo test -p livewall-engine -v`
Expected: PASS.

**Step 6: Commit**

```bash
git add crates/livewall-engine
git commit -m "feat: add runtime scheduler and performance policy"
```

### Task 8: Integrate the background service

**Files:**
- Create: `apps/livewall-service/Cargo.toml`
- Create: `apps/livewall-service/src/main.rs`
- Create: `apps/livewall-service/src/bootstrap.rs`
- Modify: `crates/livewall-control/src/lib.rs`
- Modify: `crates/livewall-engine/src/runtime.rs`

**Step 1: Write a failing service bootstrap test**

Add a small integration test or command-path test that verifies the service can initialize dependencies and answer `GetStatus`.

**Step 2: Run the bootstrap test**

Run: `cargo test -p livewall-service -v`
Expected: FAIL because the service binary and IPC server do not exist.

**Step 3: Implement service startup**

Startup order:
- initialize COM and Media Foundation
- create the shared D3D11 device
- enumerate monitors
- attach wallpaper windows
- start the named-pipe server
- start the runtime coordinator

**Step 4: Implement shutdown and crash-safe cleanup**

Ensure wallpaper windows detach cleanly, COM resources release, and the service writes structured logs to `%LOCALAPPDATA%\\LiveWall\\logs`.

**Step 5: Verify service behavior**

Run: `cargo run -p livewall-service -- --once`
Expected: initialization completes and the service prints a healthy status snapshot.

Run: `cargo test --workspace -v`
Expected: workspace tests PASS.

**Step 6: Commit**

```bash
git add apps/livewall-service crates/livewall-control crates/livewall-engine
git commit -m "feat: integrate livewall background service"
```

### Task 9: Build the settings and tray app

**Files:**
- Create: `apps/livewall-settings/Cargo.toml`
- Create: `apps/livewall-settings/src/main.rs`
- Create: `apps/livewall-settings/src/app.rs`
- Modify: `crates/livewall-control/src/lib.rs`
- Modify: `README.md`

**Step 1: Write the failing UI state tests**

Test view-model behavior for wallpaper library loading, monitor assignment changes, and performance mode updates. Keep the tests at the Rust view-model layer, not the widget layer.

**Step 2: Run the UI state tests**

Run: `cargo test -p livewall-settings -v`
Expected: FAIL because the app state and IPC client do not exist.

**Step 3: Implement the named-pipe client and view-model**

Required screens and actions:
- library grid with preview thumbnails
- monitor assignment panel
- performance mode selector
- startup toggle
- tray menu with pause/resume and quit

**Step 4: Build the `slint` UI**

Keep UI logic thin. The app should subscribe to `StatusSnapshot`, render monitor names, and send typed commands back to the service.

**Step 5: Verify the consumer flow manually**

Run: `cargo run -p livewall-settings`
Expected: the settings window loads the installed wallpaper list, can apply a wallpaper, and the tray icon can pause/resume playback.

**Step 6: Commit**

```bash
git add apps/livewall-settings crates/livewall-control README.md
git commit -m "feat: add settings and tray app"
```

### Task 10: Add samples, smoke scripts, and release packaging

**Files:**
- Create: `wallpapers/samples/coast-video/manifest.json`
- Create: `wallpapers/samples/coast-video/preview.jpg`
- Create: `wallpapers/samples/coast-video/video.mp4`
- Create: `wallpapers/samples/aurora-scene/manifest.json`
- Create: `wallpapers/samples/aurora-scene/preview.jpg`
- Create: `wallpapers/samples/aurora-scene/shaders/fullscreen.vs.cso`
- Create: `wallpapers/samples/aurora-scene/shaders/aurora.ps.cso`
- Create: `scripts/smoke-test.ps1`
- Create: `scripts/package-release.ps1`
- Modify: `README.md`
- Modify: `docs/architecture.md`

**Step 1: Write the failing packaging script assumptions**

Document expected outputs for `scripts/package-release.ps1`: release binaries, sample wallpapers, and an installable zip.

**Step 2: Add sample wallpaper packages**

Ship one video sample and one shader sample so the first-run experience works without external downloads.

**Step 3: Add smoke scripts**

`scripts/smoke-test.ps1` should:
- build release binaries
- run desktop, scene, and video smoke paths
- verify IPC status

`scripts/package-release.ps1` should:
- build `--release`
- collect binaries, assets, and licenses
- emit `dist/livewall-v1-preview.zip`

**Step 4: Document install and test flow**

Update `README.md` with developer setup, smoke-test commands, and v1 feature boundaries.

**Step 5: Verify release flow**

Run: `powershell -ExecutionPolicy Bypass -File scripts/smoke-test.ps1`
Expected: all smoke steps PASS on Windows 11.

Run: `powershell -ExecutionPolicy Bypass -File scripts/package-release.ps1`
Expected: `dist/livewall-v1-preview.zip` exists.

**Step 6: Commit**

```bash
git add wallpapers scripts README.md docs/architecture.md
git commit -m "chore: add samples smoke tests and release packaging"
```

## Release Gate Checklist

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- Manual smoke test on Windows 11 with one monitor
- Manual smoke test on Windows 11 with two monitors and mixed DPI
- Manual smoke test on laptop battery mode
- Manual smoke test while opening and closing a fullscreen game or video player

## Risks To Watch

- `WorkerW` integration is behavior-based rather than strongly supported API design; isolate it in `livewall-desktop` so replacements stay local.
- Hardware decode interop can fail on older or unusual drivers; implement clear fallback logging before considering a software decode path.
- Per-monitor sync gets tricky when the same video plays across mixed refresh-rate monitors; share the decode pipeline when possible and present independently.
- Scene packages can still crash the render path if invalid; validate package contents before activation and keep the previous wallpaper alive until the new one is healthy.

## After V1

- Add `.livewall` import/export tooling for creators
- Add wallpaper playlists and time-based rotation
- Add audio-reactive scene uniforms
- Evaluate optional HTML runtime as a separate sandboxed process only after the native runtime is stable

Plan complete and saved to `docs/plans/2026-04-29-live-wallpaper-v1.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

Which approach?
