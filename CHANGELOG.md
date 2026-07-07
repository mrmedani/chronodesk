# Changelog

All notable changes to CHRONODESK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.5] - 2026-07-07

### Fixed

- **Connexion freeze** — Ajout d'un timeout de 15s sur `create_and_send_offer` pour éviter que le transport ne bloque indéfiniment sur la négociation ICE
- **Disconnect bloquant** — Ajout d'un timeout de 5s sur `pc.close()` pour que la déconnexion ne pende jamais
- **Erreur signaling muette** — Propagation des échecs d'envoi `SignalingCommand::SendOffer` en `TransportEvent::Error` visibles dans l'UI
- **État "connecting" persistant** — Le handler `error` côté Flutter réinitialise maintenant `_connecting = false` et annule le timer
- **Boucle de polling infinie** — Ajout d'un garde-fou de 100 événements max par tick de polling pour ne jamais bloquer l'isolate Dart

## [0.4.4] - 2026-07-06

### Fixed

- :arrow_up: **BUG-UPDATE-01** — Installateur s'installe maintenant dans le dossier de l'exécutable courant (`/DIR=`), plus perdu dans Program Files
- :arrow_up: **BUG-UPDATE-02** — L'app ne se ferme plus avant la fin du script PowerShell (synchronisation par PID, plus de `exit(0)` prématuré)
- :arrow_up: **BUG-UPDATE-03** — Le script PowerShell utilise le vrai PID au lieu du nom de processus (`Get-Process -Id $targetPid`)
- :arrow_up: **BUG-UPDATE-04** — Extraction du hash SHA256 par regex locale-indépendante (fonctionne en FR, JA, CN, AR)
- :arrow_up: **BUG-UPDATE-05** — Vérification du code de sortie de l'installateur avec notification Windows ballon en cas d'échec
- :arrow_up: **BUG-UPDATE-06** — Nettoyage garanti du fichier partiel dans tous les cas d'annulation
- :arrow_up: **BUG-UPDATE-07** — Comparaison de versions pré-release conforme à la spec SemVer 2.0
- :arrow_up: **BUG-UPDATE-08** — Checksum non bloquant : fallback si le fichier .sha256 est indisponible

### Added

- :repeat: **Auto-restart** après installation réussie
- :balloon: **Notification Windows** ballon (NotifyIcon) si l'installateur échoue
- :broom: Nettoyage automatique des scripts PowerShell périmés (`_cleanupStaleScripts`)
- :id: Fichiers temporaires nommés par PID (pas de collisions)

## [0.4.0] - 2026-07-02

### Added

- :locked: **End-to-end encryption** — ECDH (X25519) key exchange at connection start, ChaCha20-Poly1305 session encryption for all subsequent messages
- :memo: **File transfer** — Chunked streaming over data channel with progress tracking, accept/reject/cancel, and download directory management
- :headphone: **Audio streaming** — Cross-platform audio capture (CPAL) with Opus/raw PCM encoding, real-time playback on viewer side
- :clipboard: **Clipboard sync** — Bidirectional clipboard text synchronization between host and viewer
- :bar_chart: **Adaptive quality** — Dynamic FPS and resolution based on round-trip time and packet loss metrics

### Changed

- :recycle: **Crypto** — New `src/crypto.rs` module: key generation, shared secret computation, ChaCha20-Poly1305 session encrypt/decrypt
- :recycle: **File transfer** — New `src/file_transfer.rs` module with `sanitize_filename()` for path traversal protection
- :recycle: **Audio** — New `src/audio.rs` module with `AudioCapture` (CPAL) + `AudioPlayer` + resampling
- :recycle: **Protocol** — `ChannelMessage` enum extended with `Handshake`, `Encrypted`, `FileTransfer*`, `AudioData`, `Clipboard` variants
- :recycle: **FFI** — `chronodesk_send_file`, `chronodesk_accept_file_transfer`, `chronodesk_reject_file_transfer`, `chronodesk_cancel_file_transfer` exported

### Fixed

- :shield: **No more `.unwrap()` panics** — all error paths now logged instead of panicking (AudioCodec, video codec, mutex poison)
- :shield: **Poison resilience** — `lock_state()` uses `unwrap_or_else(|e| e.into_inner())` throughout
- :shield: **Deadlock fix** — `chronodesk_accept()` holds mutex lock briefly, preventing deadlock with event loop
- :shield: **Path traversal** — Incoming file names sanitized via `sanitize_filename()` before creating files
- :shield: **Pixel buffer** — `chunks_exact(4)` prevents panic on non-aligned BGRA data
- :shield: **Division by zero** — Guard for `input_channels == 0` in audio resampling
- :bug: **Frame leak** — `_frameImage.dispose()` called before replacement and on disconnect
- :bug: **setState after dispose** — `Future.delayed` in `initState` now checks `mounted`
- :bug: **Keyboard capture** — Escape toggles capture mode; Tab/Alt/Meta/Windows keys pass through
- :bug: **Log truncation** — Log file no longer truncated on init, preserving pre-crash diagnostics

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
