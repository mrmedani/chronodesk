# Contributing to CHRONODESK

First off, thanks for taking the time to contribute! :tada:

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
4. **Install dependencies**: `cargo build`
5. **Run the tests**: `cargo test`

---

## Development Workflow

### Setting up your environment

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone the repo
git clone https://github.com/your-username/chronodesk.git
cd chronodesk

# Verify it builds
cargo build
```

### Running locally

```bash
# Terminal 1: Start the signaling server
cargo run --bin signaling-server

# Terminal 2: Start a host
cargo run -- --peer-id host1

# Terminal 3: Connect as a client
cargo run -- --connect host1
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

### Dart/Flutter

- Run `dart format .` before committing
- Follow the [Flutter style guide](https://docs.flutter.dev/style-guide)
- Use `const` constructors where possible

### General

- Write descriptive variable and function names
- Add doc comments for public APIs (`///`)
- Keep functions small and focused
- Handle errors, don't unwrap in production code
- Prefer immutable state where possible

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
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test test_name

# Run clippy
cargo clippy --all-targets

# Check formatting
cargo fmt --all --check
```

---

## Project Structure

```
src/
├── main.rs          — CLI entrypoint
├── lib.rs           — Library exports (for FFI)
├── capture.rs       — Screen capture module
├── crypto.rs        — Encryption module
├── input.rs         — Input injection module
├── video.rs         — Video encoding module
├── protocol.rs      — Data channel protocol
├── ffi.rs           — C FFI exports
├── network/         — WebRTC networking
│   ├── transport.rs — PeerConnection management
│   └── signaling.rs — Signaling client
└── bin/
    └── signaling.rs — Signaling server binary
```

---

## Getting Help

- Open a [Discussion](https://github.com/mrmedani/chronodesk/discussions)
- Join our community chat (coming soon)
- Check the [documentation](docs/) (in progress)

---

Again, thank you for contributing! :rocket:
