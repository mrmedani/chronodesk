<div align="center">
  <h1>CHRONODESK</h1>
  <p><strong>Open-source remote desktop — a fast, secure, self-hosted alternative to AnyDesk &amp; RustDesk</strong></p>

  <p>
    <a href="https://github.com/mrmedani/chronodesk/actions"><img src="https://img.shields.io/github/actions/workflow/status/mrmedani/chronodesk/rust.yml?branch=master&logo=github&label=build" alt="Build Status" /></a>
    <a href="https://github.com/mrmedani/chronodesk/releases"><img src="https://img.shields.io/github/v/release/mrmedani/chronodesk?include_prereleases&logo=github" alt="Release" /></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-AGPLv3-blue.svg" alt="License" /></a>
    <a href="https://github.com/mrmedani/chronodesk/issues"><img src="https://img.shields.io/github/issues/mrmedani/chronodesk?logo=github" alt="Issues" /></a>
    <br/>
    <a href="https://github.com/mrmedani/chronodesk/releases/latest/download/chronodesk-windows-setup.exe"><img src="https://img.shields.io/badge/Download%20Installer-Windows-blue?logo=windows&style=for-the-badge" alt="Download Windows Installer" /></a>
    <br/>
    <img src="https://img.shields.io/badge/Rust-1.83%2B-orange?logo=rust" alt="Rust" />
    <img src="https://img.shields.io/badge/Flutter-3.x-blue?logo=flutter" alt="Flutter" />
    <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey" alt="Platform" />
  </p>
</div>

---

## Features

| Capability | Status | Detail |
|-----------|--------|--------|
| :computer: Screen capture | Done | DXGI (Windows), multi-monitor, dirty rectangle detection (64x64 tiles) |
| :satellite: P2P transport | Done | WebRTC with ICE/STUN, NAT traversal, data channel |
| :signal_strength: Signaling server | Done | Self-hosted WebSocket broker for peer discovery & SDP relay |
| :video_camera: Video encoding | Done | H.264 (NVENC/QSV/AMF) with FFmpeg or fallback JPEG |
| :mouse: Input injection | Done | Mouse move/click, keyboard via `enigo` (Windows/macOS/Linux) |
| :art: Flutter UI | Done | Single-screen AnyDesk-like UX — peer ID, connect field, remote view, accept/deny dialog |
| :link: Rust ↔ Flutter bridge | Done | Raw C FFI with event polling, frame buffer, accept/deny flow |
| :id: ID system | Done | Persistent 9-digit peer ID stored in `%APPDATA%/chronodesk` |
| :locked: Encryption | Ready | AEAD via `ring` (chacha20-poly1305) — wired, key exchange pending |
| :clipboard: File transfer | Planning | Planned over WebRTC data channel |
| :globe_with_meridians: Remote audio | Planning | Planned via WebRTC audio tracks |

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                    CHRONODESK (Flutter App)                    │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  Flutter UI  (home_screen.dart)                          │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐   │  │
│  │  │ Peer ID  │  │ Connect  │  │ Remote Screen        │   │  │
│  │  │ Display  │  │  Field   │  │ (RawImage from RGBA) │   │  │
│  │  └──────────┘  └──────────┘  └──────────────────────┘   │  │
│  └─────────────────────────────────────────────────────────┘  │
│                           │ FFI (C ABI)                        │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  Rust Engine (chronodesk.dll)                            │  │
│  │  ┌────────────┐  ┌──────────┐  ┌────────────────────┐   │  │
│  │  │  Event     │  │  Frame   │  │  WebRTC Transport  │   │  │
│  │  │  Queue     │  │  Buffer  │  │  (webrtc crate)    │   │  │
│  │  └────────────┘  └──────────┘  └────────────────────┘   │  │
│  │  ┌────────────┐  ┌──────────┐  ┌────────────────────┐   │  │
│  │  │  Screen    │  │  Video   │  │  Signaling Client  │   │  │
│  │  │  Capture   │  │  Encoder │  │  (WebSocket)       │   │  │
│  │  └────────────┘  └──────────┘  └────────────────────┘   │  │
│  └─────────────────────────────────────────────────────────┘  │
│                           │ WebSocket                          │
└───────────────────────────┼──────────────────────────────────┘
                            │
                    ┌───────┴────────┐
                    │ Signaling Srv  │
                    │ ws://:21116/ws │
                    └────────────────┘

                        WebRTC P2P
                    ┌─────────────────┐
                    │  ICE / STUN     │
                    │  DTLS / SCTP    │
                    │  Data Channel   │
                    └─────────────────┘
```

**Protocol flow:**
1. App starts → Rust loads peer ID (persistent 9-digit), connects to signaling server
2. Enter remote ID → create WebRTC offer → send via signaling server
3. Remote receives connection request → accept/deny dialog shown
4. On accept → WebRTC handshake completes → P2P data channel opens
5. Host captures screen, encodes frames, sends over data channel
6. Viewer receives frames, decodes to RGBA, renders via Flutter `RawImage`
7. Input events flow Viewer → Host over data channel

---

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) 1.83+ (`rustup install stable`)
- [Flutter](https://flutter.dev) 3.x (for the UI)
- Visual Studio Build Tools (Windows) or CMake (Linux/macOS)

### 1. Start the Signaling Server

```bash
cargo run --bin signaling-server
```

The server listens at `ws://<host>:21116/ws`.

### 2. Build & Run the Flutter App

```bash
# Build the Rust DLL
cargo build --lib

# Copy it to the Flutter release directory
copy target\debug\chronodesk.dll chronodesk_flutter\build\windows\x64\runner\Release\

# Build & run Flutter
cd chronodesk_flutter
flutter pub get
flutter run -d windows
```

The app launches with your 9-digit peer ID. Enter another peer's ID and click **Connect**.

> **macOS / Linux**: Replace `.dll` with `.dylib` / `.so` and adjust paths accordingly.

---

## Building from Source

### Rust Engine (DLL)

```bash
# Debug DLL
cargo build --lib

# Release DLL
cargo build --release --lib

# With FFmpeg H.264 support
cargo build --release --features ffmpeg --lib
```

Output: `target/debug/chronodesk.dll` (or `.so`/`.dylib`)

### Flutter UI

```bash
cd chronodesk_flutter
flutter pub get
flutter build windows   # or macos / linux
```

### Standalone Binaries

```bash
# Signaling server
cargo build --bin signaling-server

# CLI engine (legacy host/client modes)
cargo build
```

---

## Project Structure

```
chronodesk/
├── src/                          # Rust core engine
│   ├── lib.rs                    # Library exports
│   ├── ffi.rs                    # C FFI exports (ID system, event queue, frame buffer)
│   ├── bin/signaling.rs          # WebSocket signaling server
│   ├── capture.rs                # Screen capture (xcap DXGI)
│   ├── crypto.rs                 # AEAD encryption (ring)
│   ├── input.rs                  # Input injection (enigo)
│   ├── video.rs                  # Video encoding (ffmpeg/JPEG)
│   ├── protocol.rs               # Data channel message protocol
│   ├── main.rs                   # CLI entrypoint (legacy)
│   └── network/
│       ├── transport.rs          # WebRTC PeerConnection
│       └── signaling.rs          # Signaling client (WebSocket)
├── chronodesk_flutter/           # Flutter UI
│   ├── lib/
│   │   ├── main.dart
│   │   └── src/
│   │       ├── app.dart          # App root (single screen)
│   │       ├── screens/
│   │       │   └── home_screen.dart  # AnyDesk-like single-screen UX
│   │       └── ffi/
│   │           └── native.dart   # Raw C FFI bindings
│   ├── windows/                  # Windows runner
│   └── pubspec.yaml
├── server/                       # Server infrastructure (future)
├── docs/                         # Documentation
├── .github/                      # CI/CD workflows
└── Dockerfile                    # Signaling server container
```

---

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Core engine | **Rust** — performance, safety, memory efficiency |
| P2P transport | **WebRTC** via `webrtc` crate — ICE, STUN, DTLS, SCTP |
| Screen capture | **xcap** — DXGI (Windows), CoreGraphics (macOS), X11/PipeWire (Linux) |
| Video encoding | **FFmpeg** (NVENC/QSV/AMF) or **libjpeg** fallback |
| Input injection | **enigo** — cross-platform input simulation |
| Encryption | **ring** — AEAD (ChaCha20-Poly1305) |
| UI | **Flutter** — Material Design 3, native performance |
| Bridge | **Raw C FFI** — event polling, RGBA frame buffer, JSON event queue |

---

## Roadmap

- [x] Core P2P connectivity & signaling
- [x] Screen capture & video encoding
- [x] Input injection
- [x] Flutter UI with remote screen viewer
- [x] Rust ↔ Flutter FFI bridge with event system
- [ ] End-to-end encryption (key exchange)
- [ ] File transfer over data channel
- [ ] Audio streaming
- [ ] Clipboard sync
- [ ] TURN server for restrictive NATs
- [ ] Headless mode for servers
- [ ] Mobile clients (iOS/Android)

---

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

1. Fork the repository
2. Create your feature branch (`git checkout -b feat/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feat/amazing-feature`)
5. Open a Pull Request

---

## Security

Found a vulnerability? Read [SECURITY.md](SECURITY.md) and report responsibly.

---

## License

This project is licensed under the **GNU Affero General Public License v3.0** — see [LICENSE](LICENSE).

---

<div align="center">
  <sub>Built with Rust &amp; Flutter</sub>
  <br/>
  <sub>© 2026 CHRONODESK Contributors</sub>
</div>
