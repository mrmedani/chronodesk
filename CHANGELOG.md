# Changelog

All notable changes to CHRONODESK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
