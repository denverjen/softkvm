# SoftKVM

A software KVM (Keyboard, Video, Mouse) switch that lets you share a single keyboard and mouse between two machines over a local network. Move your mouse seamlessly from one screen to another, just like a dual-monitor setup.

**Both Server and Client run on Windows 10/11 and Ubuntu (Linux).**

## How It Works

```
┌──────────────────────┐                    ┌──────────────────────┐
│  KVM Server          │    TCP / LAN       │  KVM Client          │
│  (Win or Linux)      │◄──────────────────►│  (Win or Linux)      │
│                      │   Input Events     │                      │
│  Physical KB + Mouse │   Clipboard        │  Virtual KB + Mouse  │
│  Screen 1    │       │                    │       │  Screen 2    │
│              │  ────►│                    │◄────  │              │
│              │ cross │                    │ cross │              │
│              │ edge  │                    │ edge  │              │
└──────────────────────┘                    └──────────────────────┘
```

- **Server**: The machine with the physical keyboard and mouse connected. Captures raw input events.
  - **Windows**: Uses the Windows Raw Input API
  - **Linux**: Uses `evdev` to read input devices directly
- **Client**: The machine that receives input events over the network and injects them as virtual devices.
  - **Windows**: Uses `SendInput` Win32 API
  - **Linux**: Uses `uinput` kernel module via `evdev`
- When the mouse crosses a configured screen edge, control transfers to the other machine seamlessly.
- Keyboard input follows mouse focus automatically.

## Features

- Seamless mouse transition across screen edges
- Full keyboard sharing (follows mouse focus)
- Clipboard synchronization between machines
- Low-latency binary protocol over TCP
- Configurable screen layout (left/right/top/bottom)
- Optional TLS encryption
- Auto-discovery via mDNS (optional)
- **Cross-platform**: Server and Client both support Windows 10/11 and Ubuntu Linux

## Platform Support Matrix

|           | Windows 10/11 | Ubuntu / Linux |
|-----------|:-------------:|:--------------:|
| **Server** (Input Capture) | Raw Input API | evdev |
| **Client** (Input Injection) | SendInput API | uinput |

## Requirements

| Component | Windows | Linux |
|-----------|---------|-------|
| **Server** | Windows 10 or 11 | Ubuntu 20.04+ with read access to `/dev/input/` |
| **Client** | Windows 10 or 11 | Ubuntu 20.04+ with `uinput` kernel module |
| **Network** | LAN between both machines | LAN between both machines |
| **Runtime** | Single static binary | Single binary (udev rules recommended) |

## Project Structure

```
softkvm/
├── Cargo.toml                  # Workspace root
├── README.md
├── LICENSE
├── .gitignore
├── crates/
│   ├── protocol/               # Shared protocol definitions
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── message.rs      # Message types (MouseMove, KeyDown, etc.)
│   │       └── serialize.rs    # Binary serialization
│   ├── server/                 # KVM Server binary (Win + Linux)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── capture.rs      # Input capture (Win: Raw Input, Linux: evdev)
│   │       ├── edge.rs         # Screen edge detection
│   │       ├── clipboard.rs    # Clipboard (Win: Win32 API, Linux: xclip)
│   │       └── network.rs      # TCP connection handler
│   ├── client/                 # KVM Client binary (Win + Linux)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── inject.rs       # Input injection (Win: SendInput, Linux: uinput)
│   │       ├── edge.rs         # Screen edge detection
│   │       ├── clipboard.rs    # Clipboard (Win: Win32 API, Linux: xclip)
│   │       └── network.rs      # TCP connection to server
│   └── common/                 # Shared utilities
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── config.rs       # Configuration parsing (TOML)
│           └── layout.rs       # Screen layout management
├── config/
│   └── softkvm.toml.example    # Example configuration file
└── tests/
    └── integration.rs
```

## Network Protocol

SoftKVM uses a custom binary protocol over TCP for minimal latency.

### Message Format

```
┌──────────┬──────────┬─────────────────┐
│  Magic   │  MsgType │     Payload     │
│  (2B)    │  (1B)    │   (variable)    │
└──────────┴──────────┴─────────────────┘
```

- **Magic**: `0x5F4B` (constant, for validation)
- **MsgType**: 1 byte indicating message type
- **Payload**: Variable length depending on message type

### Message Types

| ID  | Type              | Direction         | Payload                           |
|-----|-------------------|-------------------|-----------------------------------|
| 0x01| `HELLO`           | Client → Server   | Client screen resolution          |
| 0x02| `HELLO_ACK`       | Server → Client   | Server screen resolution + layout |
| 0x10| `MOUSE_MOVE`      | Bidirectional     | dx (i16), dy (i16)               |
| 0x11| `MOUSE_BUTTON`    | Bidirectional     | button (u8), state (u8)          |
| 0x12| `MOUSE_SCROLL`    | Bidirectional     | delta (i16)                       |
| 0x20| `KEY_DOWN`        | Bidirectional     | keycode (u16)                     |
| 0x21| `KEY_UP`          | Bidirectional     | keycode (u16)                     |
| 0x30| `CLIPBOARD`       | Bidirectional     | length (u32), data (bytes)        |
| 0x40| `EDGE_ENTER`      | Bidirectional     | edge (u8), position (u16)         |
| 0x41| `EDGE_LEAVE`      | Bidirectional     | edge (u8)                         |
| 0x50| `SCREEN_INFO`     | Bidirectional     | width (u16), height (u16)         |
| 0xFF| `HEARTBEAT`       | Bidirectional     | (none)                            |

### Key Code Mapping

Windows virtual key codes are mapped to Linux key codes using a built-in translation table in the protocol crate. This ensures consistent behavior across platforms.

## Screen Layout & Edge Detection

The screen layout defines the relative position of the two machines:

```
  ┌──────────┐┌──────────┐       ┌──────────┐
  │          ││          │       │          │
  │ Server   ││ Client   │       │ Server   │
  │          ││          │       │          │
  │          ││          │       │          │
  └──────────┘└──────────┘       └──────────┘
                                            ┌──────────┐
                                            │          │
                                            │ Client   │
                                            │          │
                                            │          │
                                            └──────────┘
```

Supported layouts:
- `left-right`: Client is to the right of Server (default)
- `right-left`: Client is to the left of Server
- `top-bottom`: Client is below Server
- `bottom-top`: Client is above Server

### Transition Flow

1. Mouse moves to the configured edge on the active machine
2. An `EDGE_ENTER` message is sent to the other machine
3. Subsequent `MOUSE_MOVE` events are forwarded to the target machine
4. When the mouse returns and crosses back, an `EDGE_LEAVE` is sent
5. Keyboard events are routed to whichever machine currently has mouse focus

## Configuration

Configuration is stored in `softkvm.toml`:

```toml
[server]
# IP address to listen on (0.0.0.0 = all interfaces)
listen = "0.0.0.0"
# Port number
port = 24800

[client]
# Server IP address to connect to
host = "192.168.1.100"
# Server port
port = 24800

[layout]
# How the client screen is positioned relative to the server
position = "left-right"  # left-right | right-left | top-bottom | bottom-top

[clipboard]
# Enable clipboard sharing
enabled = true
# Maximum clipboard size in bytes (default 1MB)
max_size = 1048576

[security]
# Enable TLS encryption
tls = false
```

## Usage

### Build

```bash
# Build everything
cargo build --release

# Build server only
cargo build --release -p softkvm-server

# Build client only
cargo build --release -p softkvm-client
```

### Cross-Compile

```bash
# Install cross-compilation target (e.g., build for Windows from Linux)
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc -p softkvm-server
```

### Run Server

**On Windows:**
```powershell
softkvm-server.exe --config softkvm.toml
```

**On Linux:**
```bash
# May need input group membership for evdev access
softkvm-server --config softkvm.toml
```

### Run Client

**On Windows:**
```powershell
softkvm-client.exe --config softkvm.toml
```

**On Linux:**
```bash
# uinput requires root or udev rules
sudo softkvm-client --config softkvm.toml
```

### Linux Udev Setup

To run the client without `sudo`, add a udev rule for uinput:

```bash
echo 'KERNEL=="uinput", MODE="0660", GROUP="input"' | sudo tee /etc/udev/rules.d/99-uinput.rules
sudo udevadm control --reload-rules
sudo usermod -aG input $USER
# Log out and back in
```

To allow the server to read input devices without root:

```bash
sudo usermod -aG input $USER
# Log out and back in
```

## Implementation Phases

| Phase | Scope | Status |
|-------|-------|--------|
| 1 | Project setup + protocol crate + networking | In progress |
| 2 | Server input capture (Windows Raw Input + Linux evdev) | In progress |
| 3 | Client input injection (Windows SendInput + Linux uinput) | In progress |
| 4 | Edge detection + seamless mouse transition | Not started |
| 5 | Clipboard sharing | In progress |
| 6 | Configuration (CLI args + TOML) | In progress |
| 7 | Auto-discovery (mDNS) + TLS | Not started |
| 8 | Testing, polish, release binaries | Not started |

## Key Dependencies

| Crate | Purpose | Used By |
|-------|---------|---------|
| `windows` | Win32 Raw Input, SendInput, clipboard | server (Win), client (Win) |
| `evdev` | Linux input capture / uinput injection | server (Linux), client (Linux) |
| `tokio` | Async runtime, TCP networking | server, client |
| `serde` | Serialization | protocol, common |
| `toml` | Config file parsing | common |
| `tracing` | Logging | all |
| `bytes` | Buffer management | server, client, protocol |

## License

MIT
