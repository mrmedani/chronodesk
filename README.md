<div align="center">
  <h1>CHRONODESK</h1>
  <p><strong>Open-source remote desktop — a fast, secure, self-hosted alternative to AnyDesk &amp; TeamViewer</strong></p>

  <!-- Badges -->
  <p>
    <a href="https://github.com/mrmedani/chronodesk/actions"><img src="https://img.shields.io/github/actions/workflow/status/mrmedani/chronodesk/rust.yml?branch=master&logo=github&label=build" alt="Build Status" /></a>
    <a href="https://crates.io/crates/chronodesk"><img src="https://img.shields.io/crates/v/chronodesk?logo=rust" alt="Crates.io" /></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-AGPLv3-blue.svg" alt="License" /></a>
    <a href="https://github.com/mrmedani/chronodesk/releases"><img src="https://img.shields.io/github/v/release/mrmedani/chronodesk?include_prereleases&logo=github" alt="Release" /></a>
    <a href="https://github.com/mrmedani/chronodesk/issues"><img src="https://img.shields.io/github/issues/mrmedani/chronodesk?logo=github" alt="Issues" /></a>
    <a href="https://github.com/mrmedani/chronodesk/pulls"><img src="https://img.shields.io/github/issues-pr/mrmedani/chronodesk?logo=github" alt="PRs" /></a>
    <br/>
    <img src="https://img.shields.io/badge/Rust-1.83%2B-orange?logo=rust" alt="Rust" />
    <img src="https://img.shields.io/badge/Flutter-3.x-blue?logo=flutter" alt="Flutter" />
    <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey" alt="Platform" />
    <img src="https://img.shields.io/badge/PRs-welcome-brightgreen" alt="PRs Welcome" />
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
| :locked: Encryption | Ready | AEAD via `ring` (chacha20-poly1305) — wired, key exchange pending |
| :clipboard: File transfer | Planning | Planned over WebRTC data channel |
| :globe_with_meridians: Remote audio | Planning | Planned via WebRTC audio tracks |
| :iphone: Cross-platform UI | In progress | Flutter front-end with `flutter_rust_bridge` |

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     CHRONODESK                           │
│                                                          │
│  ┌──────────────┐           WebRTC           ┌──────────┐│
│  │  Host         │◄────── SCTP/DTLS ───────►│  Client  ││
│  │  (Rust Core)  │                           │(Rust+Flut)│
│  └───┬───┬───┬──┘                           └────┬──┬───┘
│      │   │   │                                    │  │
│      │   │   └──► Screen Capture (xcap)          │  │
│      │   │       └──► Video Encode (ffmpeg/JPEG) │  │
│      │   │           └──► Send via DataChannel   │  │
│      │   │                                        │  │
│      │   └──────────► Receive Input Events ◄──────┘  │
│      │                                                │
│      └──────────────────► WebSocket Signaling ◄───────┘
│                                │
│                     ┌──────────┴──────────┐
│                     │  Signaling Server   │
│                     │  (WebSocket Broker) │
│                     └─────────────────────┘
```

**Protocol flow:**
1. Peers register with the signaling server via WebSocket
2. Host creates an SDP offer, sent via signaling server to the client
3. Client creates an SDP answer, ICE candidates are exchanged
4. Direct P2P WebRTC connection established
5. Screen frames flow Host → Client over data channel
6. Input events flow Client → Host over data channel

---

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) 1.83+ (`rustup install stable`)
- [Flutter](https://flutter.dev) 3.x (for the UI client)
- **Optional:** [FFmpeg](https://ffmpeg.org/) shared libraries (for H.264 hardware encoding)

### 1. Start the Signaling Server

```bash
cargo run --bin signaling-server -- --bind 0.0.0.0:21116
```

The server listens for WebSocket connections at `ws://<host>:21116/ws`.

### 2. Host Mode (share your screen)

```bash
cargo run -- --peer-id mydesk
```

Or with H.264 hardware encoding (requires FFmpeg):

```bash
cargo run --features ffmpeg -- --peer-id mydesk
```

### 3. Client Mode (connect to a host)

```bash
cargo run -- --connect mydesk
```

### CLI Options

```
cargo run -- [FLAGS] [OPTIONS]

Modes:
  host        Share your screen (default when --peer-id is set)
  client      Connect to a remote host (use --connect)
  server      Run the signaling server (use cargo run --bin signaling-server)

Options:
  --peer-id <ID>        Set your peer identifier (auto-generated if omitted)
  --connect <ID>        Connect to a remote peer
  --signaling <ADDR>    Signaling server address [default: 127.0.0.1:21116]
  --bind <ADDR>         Bind address for signaling server [default: 0.0.0.0:21116]
```

---

## Building from Source

### Rust Engine

```bash
# Debug build
cargo build

# Release build
cargo build --release

# With FFmpeg H.264 support
cargo build --release --features ffmpeg
```

### Flutter UI

```bash
cd chronodesk_flutter
flutter pub get
flutter build windows   # or macos / linux / ios / android
```

### Generate FFI Bindings

```bash
flutter_rust_bridge_codegen generate
```

---

## Project Structure

```
chronodesk/
├── src/                          # Rust core engine
│   ├── main.rs                   # CLI entrypoint
│   ├── lib.rs                    # Library exports
│   ├── bin/signaling.rs          # WebSocket signaling server
│   ├── capture.rs                # Screen capture (xcap DXGI)
│   ├── crypto.rs                 # AEAD encryption (ring)
│   ├── input.rs                  # Input injection (enigo)
│   ├── video.rs                  # Video encoding (ffmpeg/JPEG)
│   ├── protocol.rs               # Data channel message protocol
│   ├── ffi.rs                    # C FFI exports for Flutter
│   └── network/
│       ├── transport.rs          # WebRTC PeerConnection
│       └── signaling.rs          # Signaling client
├── chronodesk_flutter/           # Cross-platform Flutter UI
│   └── lib/
│       ├── main.dart
│       ├── src/
│       │   ├── app.dart
│       │   ├── screens/          # Home, Host, Viewer screens
│       │   └── ffi/native.dart   # Rust FFI bindings
├── server/                       # Server infrastructure (future)
├── docs/                         # Documentation (future)
└── build/                        # Build scripts
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
| Bridge | **flutter_rust_bridge** — zero-copy FFI |

---

## Roadmap

- [x] Core P2P connectivity & signaling
- [x] Screen capture & video encoding
- [x] Input injection
- [ ] End-to-end encryption (key exchange)
- [ ] Flutter UI with remote screen viewer
- [ ] File transfer over data channel
- [ ] Audio streaming
- [ ] Clipboard sync
- [ ] TURN server for restrictive NATs
- [ ] Headless mode for headless servers
- [ ] Mobile clients (iOS/Android)

---

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

1. Fork the repository
2. Create your feature branch (`git checkout -b feat/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feat/amazing-feature`)
5. Open a Pull Request

---

## Security

Found a vulnerability? Please read [SECURITY.md](SECURITY.md) and report responsibly.

---

## License

This project is licensed under the **GNU Affero General Public License v3.0** — see the [LICENSE](LICENSE) file for details.

The AGPL-3.0 ensures that:
- The source code remains open
- Modifications must be shared when providing network services
- Commercial use is permitted if you comply with the license terms

---

<div align="center">
  <sub>Built with ❤️ using Rust &amp; Flutter</sub>
  <br/>
  <sub>© 2026 CHRONODESK Contributors</sub>
</div>
