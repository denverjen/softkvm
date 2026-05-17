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

### Hardware

- Two machines connected to the **same local network (LAN)**
- Keyboard and mouse physically connected to the **Server** machine
- A display on each machine

### Operating System

| Machine | Minimum OS |
|---------|-----------|
| Server (Windows) | Windows 10 (Build 1903+) or Windows 11 |
| Server (Linux) | Ubuntu 20.04 LTS or later (kernel 5.4+) |
| Client (Windows) | Windows 10 (Build 1903+) or Windows 11 |
| Client (Linux) | Ubuntu 20.04 LTS or later (kernel 5.4+) |

### Network

- Both machines on the same LAN (Ethernet or Wi-Fi)
- TCP port **24800** open between machines (configurable)
- Recommended: Gigabit Ethernet for lowest latency

## Installation

### Download Pre-built Binaries (Recommended)

Download from the [Latest Release](https://github.com/denverjen/softkvm/releases/latest):

| File | Platform | Role |
|------|----------|------|
| `softkvm-server-linux-x86_64` | Linux x86_64 | Server |
| `softkvm-client-linux-x86_64` | Linux x86_64 | Client |
| `softkvm-server-windows-x86_64.exe` | Windows 10/11 x86_64 | Server |
| `softkvm-client-windows-x86_64.exe` | Windows 10/11 x86_64 | Client |

### Windows Setup

**No pre-installed packages required.** The binaries are standalone executables.

1. Download `softkvm-server-windows-x86_64.exe` and/or `softkvm-client-windows-x86_64.exe`
2. Place them in any folder (e.g. `C:\SoftKVM\`)
3. Run from PowerShell or Command Prompt:
   ```
   softkvm-server-windows-x86_64.exe
   ```

> **Note:** Windows Defender or antivirus may flag the binary on first run. Click "More info" → "Run anyway" to proceed.

### Linux Setup

#### Prerequisites

```bash
# Install xclip (required for clipboard sharing)
sudo apt-get update
sudo apt-get install -y xclip

# Verify uinput kernel module is loaded (required for client)
sudo modprobe uinput
lsmod | grep uinput
```

#### Install

```bash
# Download
wget https://github.com/denverjen/softkvm/releases/latest/download/softkvm-server-linux-x86_64
wget https://github.com/denverjen/softkvm/releases/latest/download/softkvm-client-linux-x86_64

# Make executable
chmod +x softkvm-server-linux-x86_64 softkvm-client-linux-x86_64
```

#### Permissions Setup (Required for Client)

The client needs access to `/dev/uinput` to create virtual input devices. Run **one** of these options:

**Option A: Udev rule (recommended, persistent)**
```bash
echo 'KERNEL=="uinput", MODE="0660", GROUP="input"' | sudo tee /etc/udev/rules.d/99-uinput.rules
sudo udevadm control --reload-rules
sudo udevadm trigger
sudo usermod -aG input $USER
# Log out and log back in for group change to take effect
```

**Option B: Run with sudo (quick test)**
```bash
sudo ./softkvm-client-linux-x86_64
```

#### Permissions Setup (Required for Server on Linux)

The server needs read access to `/dev/input/event*` devices:

```bash
sudo usermod -aG input $USER
# Log out and log back in for group change to take effect
```

Verify access:
```bash
ls -l /dev/input/
# You should see "input" group on event devices
groups $USER
# "input" should be listed
```

## Quick Start

### 1. Find your IP addresses

**On the Server machine:**
```bash
# Linux
ip addr show | grep "inet "
# Windows (PowerShell)
ipconfig | findstr "IPv4"
```

### 2. Create a config file

Create `softkvm.toml` on both machines:

```toml
[server]
listen = "0.0.0.0"
port = 24800

[client]
host = "192.168.1.100"   # Replace with your SERVER IP
port = 24800

[layout]
position = "left-right"  # Client is to the right of Server

[clipboard]
enabled = true
max_size = 1048576

[security]
tls = false
```

### 3. Start the Server

**Windows:**
```powershell
softkvm-server-windows-x86_64.exe
```

**Linux:**
```bash
./softkvm-server-linux-x86_64
```

### 4. Start the Client

**Windows:**
```powershell
softkvm-client-windows-x86_64.exe
```

**Linux:**
```bash
# If udev rules are set up:
./softkvm-client-linux-x86_64

# Otherwise:
sudo ./softkvm-client-linux-x86_64
```

### 5. Verify connection

You should see log output like:
```
INFO SoftKVM Server starting on 0.0.0.0:24800
INFO Listening on 0.0.0.0:24800
INFO Client connected from 192.168.1.101
```

```
INFO SoftKVM Client connecting to 192.168.1.100:24800
INFO Connected to server
```

Move your mouse to the right edge of the server screen — it should appear on the client screen.

## Troubleshooting

| Problem | Platform | Solution |
|---------|----------|----------|
| "No input devices found" | Linux Server | Run `sudo usermod -aG input $USER` and re-login |
| "Failed to create input injector" | Linux Client | Run `sudo modprobe uinput` |
| "Permission denied /dev/uinput" | Linux Client | Set up udev rules or run with `sudo` |
| Client can't connect | Both | Check firewall allows TCP port 24800 |
| Mouse doesn't cross edge | Both | Verify `layout.position` matches physical screen arrangement |
| Clipboard not syncing | Linux | Install `xclip`: `sudo apt install xclip` |
| High latency | Both | Use Ethernet instead of Wi-Fi; check network quality |
| Windows SmartScreen block | Windows | Click "More info" → "Run anyway" |

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

Configuration is stored in `softkvm.toml`. See [Quick Start](#quick-start) for a complete example.

### Layout Options

| Value | Description |
|-------|-------------|
| `left-right` | Client is to the right of Server (default) |
| `right-left` | Client is to the left of Server |
| `top-bottom` | Client is below Server |
| `bottom-top` | Client is above Server |

## Build from Source

<details>
<summary>Click to expand build instructions</summary>

### Prerequisites

- Rust 1.95+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- On Linux: `sudo apt install libx11-dev libevdev-dev`

### Build

```bash
git clone https://github.com/denverjen/softkvm.git
cd softkvm
cargo build --release
# Binaries at: target/release/softkvm-server, target/release/softkvm-client
```

### Cross-Compile for Windows from Linux

```bash
rustup target add x86_64-pc-windows-gnu
cargo install cargo-zigbuild
cargo zigbuild --release --target x86_64-pc-windows-gnu
```

</details>

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
