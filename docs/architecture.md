# LiveWall Architecture

## V1 Scope

LiveWall v1 targets Windows 11 only and ships as a consumer desktop app. The
runtime supports two wallpaper kinds:

- `video`: hardware-decoded playback through Media Foundation with D3D11 interop
- `scene`: GPU-rendered procedural or authored scenes backed by precompiled
  shaders

The following are explicitly out of scope for v1:

- HTML or browser-backed wallpapers
- marketplace or workshop features
- audio-reactive scenes
- cloud sync
- wallpaper authoring tools

## Process Model

The product is split into two top-level applications:

- `livewall-service`: a long-running background process that owns wallpaper
  attachment, decode, rendering, scheduling, and system integration
- `livewall-settings`: a separate consumer UI for library management, monitor
  assignment, startup preferences, and performance controls

This split is intentional. Wallpaper playback should survive settings UI
restarts, and the settings app should never host wallpaper content directly.

## Service Responsibilities

`livewall-service` is the only process that touches the desktop runtime:

- attach one wallpaper host window per monitor behind the desktop icons
- create and reuse a shared D3D11 device across monitors
- decode video wallpapers through Media Foundation hardware paths
- render scene wallpapers from packaged precompiled shader assets
- pause or throttle playback when a fullscreen application is active
- reduce work on battery or power-saver policy changes
- expose a typed IPC surface for status, assignment, and control commands

This process is also the right place for tray integration because it already
owns the lifecycle and playback state.

## Settings Responsibilities

`livewall-settings` is an operator surface, not part of the render loop. It
should:

- enumerate installed wallpapers and monitor topology
- preview packages and assign wallpapers per monitor
- apply global performance mode changes
- toggle startup behavior
- open quickly and exit without affecting active wallpapers

The UI can be replaced or redesigned later without forcing changes to the core
rendering runtime.

## Package and Content Model

Wallpapers are packaged assets with a manifest and preview media. v1 package
types are:

- `video` packages: manifest, preview image, and a video entry file
- `scene` packages: manifest, preview image, vertex shader, pixel shader, and
  optional scene config

Imported packages install into a per-user local application data directory so
the service can load them without elevated privileges.

## Shared Crate Boundaries

The planned workspace members map to clear responsibilities:

- `crates/livewall-control`: IPC contracts, DTOs, protocol versioning
- `crates/livewall-pack`: package manifest parsing, validation, installation
- `crates/livewall-desktop`: desktop attachment, WorkerW integration, monitor
  enumeration
- `crates/livewall-render`: D3D11 device ownership and scene rendering
- `crates/livewall-video`: Media Foundation playback clock and decode path
- `crates/livewall-engine`: policy layer that coordinates render, decode, and
  desktop state

This separation keeps OS integration, media, rendering, and control surfaces
testable in isolation.

## Performance Boundaries

The lightweight target depends more on policy than on raw rendering speed.
LiveWall v1 should optimize for:

- near-zero idle CPU when wallpapers are paused
- source-rate playback on AC power unless policy reduces it
- one GPU device shared across monitors
- no browser runtime in the default path
- minimal background UI footprint outside the service process

That is the main architectural difference from a broader "everything runtime"
approach.

## Runtime Reliability and Degraded Mode

Some Windows systems cannot provide the full desktop/runtime stack (for example,
missing `WorkerW` host discovery or unsupported D3D11 feature levels). The
service should keep operating in degraded mode instead of crashing:

- synthesize a monitor snapshot when desktop integration is unavailable
- continue startup when D3D11 initialization fails and synthetic mode is enabled
- keep IPC online so settings and diagnostics remain usable

This keeps the control plane responsive while making rendering capability
failures explicit in logs.

## IPC Performance Model

Service IPC uses blocking named-pipe request/response handling:

- no polling loops in idle state
- one connected client request handled at a time
- explicit disconnect after each response to keep the protocol simple

Blocking I/O here is intentional; it minimizes CPU wakeups and avoids
background spin while still providing immediate command handling.
