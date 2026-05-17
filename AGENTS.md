# SoftKVM Project Guide

## Project Overview

SoftKVM is a software KVM (Keyboard, Video, Mouse) switch written in Rust. It shares a single keyboard and mouse between two machines over a local network with seamless screen-edge transitions.

- **Server**: Captures physical keyboard/mouse input, sends events to client over TCP
- **Client**: Receives events, injects them as virtual input devices
- Both server and client support **Windows 10/11** and **Ubuntu Linux**

## Architecture

```
Workspace (Cargo.toml)
├── crates/protocol   — Binary protocol: message types, serialization (no platform deps)
├── crates/common     — Config (TOML), screen layout logic
├── crates/server     — KVM Server binary (input capture)
├── crates/client     — KVM Client binary (input injection)
```

## Platform-Specific Code

Platform differences are handled via `#[cfg(target_os = "windows")]` and `#[cfg(target_os = "linux")]`:

| Module | Windows | Linux |
|--------|---------|-------|
| Server capture | `windows` crate — Raw Input API (`WM_INPUT`) | `evdev` crate — `Device::fetch_events()` |
| Client inject | `windows` crate — `SendInput` Win32 API | `evdev` crate — `uinput::VirtualDevice` |
| Clipboard (both) | Win32 Clipboard API | `xclip` subprocess |

## Build & Check

```bash
cargo check                # Verify compilation
cargo build --release      # Build all
cargo build -p softkvm-server   # Server only
cargo build -p softkvm-client   # Client only
```

Rust toolchain: stable (1.95+). `cargo check` should pass with only unused-code warnings for scaffolding modules.

## Key Files

- `crates/protocol/src/message.rs` — All protocol message types
- `crates/protocol/src/serialize.rs` — Binary encode/decode for TCP framing
- `crates/server/src/capture.rs` — Platform input capture
- `crates/client/src/inject.rs` — Platform input injection
- `crates/common/src/config.rs` — `softkvm.toml` parsing

## Code Style

- No comments unless explicitly requested
- Follow existing patterns in the crate
- Use `anyhow::Result` for error handling
- Use `tracing` for logging (info/warn/error/debug)
- Platform-specific code goes in the same file with `#[cfg(...)]` blocks, not separate files

## Network Protocol

Binary over TCP. Header: 2B magic (`0x5F4B`) + 1B type + 1B reserved + variable payload. See `crates/protocol/src/serialize.rs`.

## Git

- Remote: `https://github.com/denverjen/softkvm.git`
- Branch: `main`
- Commit messages: concise, lowercase, imperative mood
