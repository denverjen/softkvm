# SoftKVM Project Guide

## Project Overview

SoftKVM is a software KVM (Keyboard, Video, Mouse) switch written in Rust. It shares a single keyboard and mouse between two machines over a local network with seamless screen-edge transitions.

- **Server**: Captures physical keyboard/mouse input, detects screen edge, forwards events to client over TCP
- **Client**: Receives events, injects them as virtual input devices
- Both server and client support **Windows 10/11** and **Ubuntu Linux 20.04+**

## Architecture

```
Workspace (Cargo.toml)
├── crates/protocol   — Binary protocol: message types, serialization (no platform deps)
├── crates/common     — Config (TOML), screen layout logic
├── crates/server     — KVM Server binary (input capture + edge detection + forwarding)
├── crates/client     — KVM Client binary (input injection)
```

### Server Flow

1. Listens for TCP client connections
2. Reads `Hello` from client, replies with `HelloAck` (screen size + layout)
3. Starts input capture thread (evdev on Linux, Raw Input on Windows)
4. Tracks absolute cursor position from relative mouse events
5. Detects screen edge hit based on layout config
6. On edge: sends `EdgeEnter` to client, switches to forwarding mode
7. In forwarding mode: sends all `MouseMove`, `KeyDown`, `KeyUp`, `MouseScroll` to client

### Client Flow

1. Connects to server, sends `Hello` with screen info
2. Receives `HelloAck`, receives `EdgeEnter` and input events
3. Injects received events via `uinput` (Linux) or `SendInput` (Windows)

## Platform-Specific Code

Platform differences are handled via `#[cfg(target_os = "windows")]` and `#[cfg(target_os = "linux")]`:

| Module | Windows | Linux |
|--------|---------|-------|
| Server capture | `windows` crate — Raw Input API (`WM_INPUT`) | `evdev` crate — `Device::fetch_events()` |
| Server screen size | `GetSystemMetrics` (Win32) | `XDefaultScreenOfDisplay` (X11) |
| Client inject | `windows` crate — `SendInput` Win32 API | `evdev` crate — `uinput::VirtualDevice` |
| Clipboard (both) | Win32 Clipboard API | `xclip` subprocess |

## Build & Check

```bash
cargo check                     # Verify compilation (all platforms)
cargo build --release           # Build all (native)
cargo build -p softkvm-server   # Server only
cargo build -p softkvm-client   # Client only
```

Rust toolchain: stable (1.95+). `cargo check` should pass with only unused-code warnings for scaffolding modules.

## Cross-Compilation & Release Build

### Linux (Ubuntu 20.04+ compatible, GLIBC 2.30)

Built inside a Docker container to ensure maximum compatibility:

```bash
# Prerequisites: Docker must be installed and accessible
docker run --rm -v $(pwd):/src -w /src ubuntu:20.04 bash -c \
  "DEBIAN_FRONTEND=noninteractive apt-get update && \
   DEBIAN_FRONTEND=noninteractive apt-get install -y curl build-essential pkg-config libudev-dev libx11-dev libxi-dev libxrandr-dev && \
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
   . /root/.cargo/env && \
   cargo build --release -p softkvm-server && \
   cargo build --release -p softkvm-client"
```

Output binaries: `target/release/softkvm-server`, `target/release/softkvm-client`

Verify GLIBC compatibility: `objdump -T target/release/softkvm-server | grep GLIBC | sort -V -u | tail -3` (should show 2.30 or lower)

### Windows (cross-compiled from Linux)

```bash
# Prerequisites: mingw-w64 must be installed
sudo apt-get install -y mingw-w64
rustup target add x86_64-pc-windows-gnu

cargo build --release -p softkvm-server --target x86_64-pc-windows-gnu
cargo build --release -p softkvm-client --target x86_64-pc-windows-gnu
```

Output binaries: `target/x86_64-pc-windows-gnu/release/softkvm-server.exe`, `target/x86_64-pc-windows-gnu/release/softkvm-client.exe`

## GitHub Release Process

### Prerequisites

Docker socket must be accessible: `sudo chmod 666 /var/run/docker.sock`

### Steps

1. **Build Linux binaries** (Docker Ubuntu 20.04 — see above)
2. **Build Windows binaries** (cross-compile — see above)
3. **Commit and tag**:
   ```bash
   git add -A && git commit -m "describe changes"
   git tag v0.1.X
   git push origin main v0.1.X
   ```
4. **Create GitHub release** (via API with token):
   ```bash
   TOKEN="ghp_xxx"
   curl -X POST -H "Authorization: token $TOKEN" -H "Content-Type: application/json" \
     -d '{"tag_name":"v0.1.X","name":"SoftKVM v0.1.X","body":"...release notes..."}' \
     https://api.github.com/repos/denverjen/softkvm/releases
   ```
5. **Upload binaries** (using release ID from step 4):
   ```bash
   RELEASE_ID=123456
   for f in softkvm-server-linux-x86_64 softkvm-client-linux-x86_64 \
            softkvm-server-windows-x86_64.exe softkvm-client-windows-x86_64.exe; do
     curl -X POST -H "Authorization: token $TOKEN" -H "Content-Type: application/octet-stream" \
       --data-binary @path/to/$f \
       "https://uploads.github.com/repos/denverjen/softkvm/releases/$RELEASE_ID/assets?name=$f"
   done
   ```

### Release Naming Convention

| File | Description |
|------|-------------|
| `softkvm-server-linux-x86_64` | Linux server (GLIBC 2.30+, Ubuntu 20.04+) |
| `softkvm-client-linux-x86_64` | Linux client (GLIBC 2.30+, Ubuntu 20.04+) |
| `softkvm-server-windows-x86_64.exe` | Windows 10/11 server |
| `softkvm-client-windows-x86_64.exe` | Windows 10/11 client |

## Debugging

```bash
# Server with debug logging
RUST_LOG=debug ./softkvm-server

# Client with debug logging
RUST_LOG=debug sudo ./softkvm-client

# Check GLIBC requirements of a binary
objdump -T <binary> | grep GLIBC | sed 's/.*GLIBC_//' | sed 's/).*//' | sort -V -u

# Test server locally (both on same machine)
# Terminal 1: ./softkvm-server
# Terminal 2: sudo ./softkvm-client   (needs sudo for uinput on Linux)
```

## Key Files

- `crates/protocol/src/message.rs` — All protocol message types
- `crates/protocol/src/serialize.rs` — Binary encode/decode for TCP framing
- `crates/server/src/capture.rs` — Platform input capture, cursor position tracking
- `crates/server/src/network.rs` — Server main loop: handshake, capture, edge detection, forwarding
- `crates/server/src/edge.rs` — Edge detection logic (LayoutPosition → Edge mapping)
- `crates/client/src/inject.rs` — Platform input injection
- `crates/client/src/network.rs` — Client main loop: connect, receive, inject
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
