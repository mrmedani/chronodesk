# Contributing to CHRONODESK

First off, thanks for taking the time to contribute!

The following is a set of guidelines for contributing to CHRONODESK. These are mostly guidelines, not rules. Use your best judgment, and feel free to propose changes to this document.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Pull Request Guidelines](#pull-request-guidelines)
- [Coding Standards](#coding-standards)
- [Commit Message Convention](#commit-message-convention)
- [Testing](#testing)
- [Project Structure](#project-structure)

---

## Code of Conduct

This project and everyone participating in it is governed by the [CHRONODESK Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

---

## Getting Started

1. **Fork** the repository
2. **Clone** your fork: `git clone https://github.com/your-username/chronodesk.git`
3. **Create a branch**: `git checkout -b feat/your-feature-name`
4. **Build the Rust library**: `cargo build --lib`
5. **Build the Flutter app**: see [Flutter UI](#flutter-ui) below

---

## Development Workflow

### Setting up your environment

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Flutter (Windows/macOS/Linux)
# See https://flutter.dev/docs/get-started/install

# Clone the repo
git clone https://github.com/your-username/chronodesk.git
cd chronodesk

# Build Rust core library
cargo build --lib

# Build signaling server binary
cargo build --bin signaling-server
```

### Running locally

```bash
# Terminal 1: Start the signaling server
cargo run --bin signaling-server

# Terminal 2: Build DLL and run Flutter app
cargo build --lib

# Windows
copy target\debug\chronodesk.dll chronodesk_flutter\build\windows\x64\runner\Release\
cd chronodesk_flutter
flutter run -d windows

# macOS/Linux
cp target/debug/libchronodesk.so chronodesk_flutter/build/linux/x64/release/bundle/
cd chronodesk_flutter
flutter run -d linux
```

### Flutter UI

```bash
cd chronodesk_flutter

# Get dependencies
flutter pub get

# Analyze
flutter analyze

# Build for Windows
flutter build windows

# Build for Linux
flutter build linux

# Build for macOS
flutter build macos
```

---

## Pull Request Guidelines

1. **Keep PRs focused** — one feature or fix per PR
2. **Write tests** for new functionality
3. **Update documentation** for API changes
4. **Ensure CI passes** — all checks must be green
5. **Keep the commit history clean** — rebase before opening the PR
6. **Reference issues** — use `Fixes #123` or `Closes #456` in the description

---

## Coding Standards

### Rust

- Run `cargo fmt` before committing
- Ensure `cargo clippy --all-targets -- -D warnings` passes
- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `anyhow::Result` for fallible functions
- Use `thiserror` for library error types
- Prefer `tracing` over `println`/`eprintln`
- FFI exports must use `extern "C"` and raw C types (`*const c_char`, `*mut u8`, etc.)
- Memory allocated in Rust for FFI must be freed by the corresponding `chronodesk_free_*` function

### Dart/Flutter

- Run `dart format .` before committing
- Follow the [Flutter style guide](https://docs.flutter.dev/style-guide)
- Use `const` constructors where possible
- Import `dart:ffi` and `package:ffi/ffi.dart` for native bindings
- Use `DynamicLibrary.open` for loading the Rust DLL

### General

- Write descriptive variable and function names
- Add doc comments for public APIs (`///`)
- Keep functions small and focused
- Handle errors, don't unwrap in production code
- Prefer immutable state where possible
- Never commit secrets, keys, or `*.dll` binaries

---

## Commit Message Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]
[optional footer]
```

### Types

| Type | Usage |
|------|-------|
| `feat` | A new feature |
| `fix` | A bug fix |
| `docs` | Documentation changes |
| `style` | Code style (formatting, etc.) |
| `refactor` | Code refactoring |
| `perf` | Performance improvements |
| `test` | Adding or fixing tests |
| `chore` | Build process, CI, dependencies |

### Examples

```
feat(capture): add multi-monitor support
fix(transport): handle ICE candidate timeouts
docs(readme): update quick start guide
refactor(video): simplify encoder selection logic
```

---

## Testing

```bash
# Rust tests
cargo test
cargo test -- --nocapture

# Rust lints
cargo clippy --all-targets
cargo fmt --all --check

# Flutter analyze
cd chronodesk_flutter && flutter analyze
```

---

## Project Structure

```
chronodesk/
├── src/                          # Rust core engine
│   ├── lib.rs                    # Library exports
│   ├── ffi.rs                    # C FFI exports (event queue, frame buffer, accept/deny)
│   ├── bin/signaling.rs          # WebSocket signaling server
│   ├── capture.rs                # Screen capture (xcap DXGI)
│   ├── crypto.rs                 # AEAD encryption (ring)
│   ├── input.rs                  # Input injection (enigo)
│   ├── video.rs                  # Video encoding (ffmpeg/JPEG)
│   ├── protocol.rs               # Data channel message protocol (bincode)
│   └── network/
│       ├── transport.rs          # WebRTC PeerConnection
│       └── signaling.rs          # Signaling client (WebSocket)
├── chronodesk_flutter/           # Flutter UI
│   └── lib/src/
│       ├── app.dart
│       ├── screens/home_screen.dart  # Single-screen AnyDesk-like UX
│       └── ffi/native.dart           # Raw C FFI bindings
├── server/                       # Server infrastructure (future)
├── docs/                         # Documentation
├── .github/                      # CI/CD workflows, issue/PR templates
└── Dockerfile                    # Signaling server container
```

---

## Getting Help

- Open a [Discussion](https://github.com/mrmedani/chronodesk/discussions)
- Join our community chat (coming soon)
- Check the [documentation](docs/) (in progress)

---

Again, thank you for contributing!
