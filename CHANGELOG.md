# Changelog

All notable changes to CHRONODESK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.2] - 2026-06-28

### Fixed

- :arrow_up: Update download now shows progress bar (percentage + linear indicator)
- :package: Fixed zip extraction — uses .NET ZipFile instead of unreliable PowerShell Expand-Archive
- :wrench: Update no longer silently fails on paths with spaces
- :repeat: App now properly restarts after update

## [0.2.1] - 2026-06-28

### Fixed

- :bug: Host-side `capture_active` always `false` due to `pending_offer` consumed in `chronodesk_accept()` before `Connected` event — now uses explicit `is_host` flag
- :wrench: Default signaling server address set to `144.24.201.196:21116` (Oracle Cloud)

### Added

- :arrow_up: Auto-update checker — checks GitHub Releases on startup, prompts user to update
- :package: Automatic update flow — downloads zip, extracts, restarts app silently

## [0.2.0] - 2026-06-27

### Added

- :id: Persistent 9-digit peer ID system (stored in `%APPDATA%/chronodesk/id`)
- :art: Single-screen AnyDesk-like Flutter UX — peer ID display, connect field, remote screen view
- :link: Raw C FFI bridge with event queue (JSON Rust→Flutter), RGBA frame buffer, accept/deny flow
- :incoming_envelope: Connection request dialog (accept/deny incoming connections from Flutter)
- :frame_photo: Remote screen rendering via `ui.decodeImageFromPixels` + `RawImage` (30 FPS polling)
- :arrow_forward: `SendMessage` transport command for sending input/data channel messages from FFI
- :satellite: ICE candidate forwarding from WebRTC to signaling client via `signaling_tx` channel
- :package: `ffi` Dart package dependency for `calloc`/`NativeUtf8` helpers

### Changed

- :recycle: Rewrote `src/ffi.rs` — full integration of transport, signaling, capture, encoding in single event loop
- :recycle: Rewrote `chronodesk_flutter/lib/src/screens/home_screen.dart` — single-screen UX (replaces separate host/viewer screens)
- :recycle: Rewrote `chronodesk_flutter/lib/src/ffi/native.dart` — all 12 FFI functions bound
- :wrench: Fixed `src/network/transport.rs` — SDP routing to signaling client instead of internal loopback
- :wrench: Simplified `chronodesk_flutter/lib/src/app.dart` — single route
- :book: Updated README with new architecture diagram, FFI-based flow, updated project structure

## [0.1.0] - 2026-06-26

### Added

- :satellite: WebSocket signaling server with peer registration, SDP relay, and ICE candidate forwarding
- :computer: Screen capture module using `xcap` with DXGI support on Windows, multi-monitor, and 64×64 tile dirty rectangle detection
- :signal_strength: WebRTC P2P transport with ICE/STUN, data channel communication, and signaling client integration
- :video_camera: Video encoding pipeline with JPEG (default) and optional H.264 via FFmpeg (NVENC/QSV/AMF)
- :mouse: Input injection module using `enigo` for cross-platform mouse and keyboard simulation
- :package: Binary protocol for data channel messaging (video frames, input events, clipboard, ping/pong)
- :link: C FFI exports (`start_host`, `start_client`) for Flutter integration via `flutter_rust_bridge`
- :art: Flutter UI scaffold with three screens: Home, Host (screen sharing), and Viewer (remote connection)
- :building_construction: Modular project structure with `src/capture`, `src/network`, `src/video`, `src/input`, and more
- :white_check_mark: CI pipeline with GitHub Actions (check, build, test on Ubuntu/Windows/macOS)
- :page_facing_up: Issue templates (bug report, feature request), PR template, and contributing guidelines

### Build

- Cargo.toml with `ffmpeg-next` as optional feature
- `[lib]` target as `cdylib`+`staticlib` for Flutter FFI
- `signaling-server` binary target
- Release profile with LTO and single codegen unit

---

For a full list of commits, see the [GitHub commit log](https://github.com/mrmedani/chronodesk/commits/master).
