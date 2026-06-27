# CHRONODESK

Open-source remote desktop software — a fast, secure, self-hosted alternative to AnyDesk/TeamViewer.

Built with **Rust** (core engine) and **Flutter** (cross-platform UI).

## Architecture

```
Client (Rust + Flutter)  ◄──WebRTC──►  Client (Rust + Flutter)
         │                                    │
         └────────── Signaling ───────────────┘
                     Server
```

## Features (WIP)

- [ ] P2P remote desktop via WebRTC
- [ ] Hardware-accelerated video encoding (NVENC/QSV/AMF)
- [ ] End-to-end encryption
- [ ] NAT traversal (STUN/TURN/ICE)
- [ ] Cross-platform (Windows, macOS, Linux)
- [ ] File transfer
- [ ] Self-hosted signaling server

## Build

```bash
cargo build --release
```

## License

AGPL-3.0
