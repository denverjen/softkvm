use anyhow::Result;
use bytes::BytesMut;
use softkvm_common::Config;
use softkvm_protocol::message::*;
use softkvm_protocol::serialize;
use softkvm_protocol::Message;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::capture::{self, CaptureEvent};
use crate::edge::{EdgeDetector, FocusTarget};

pub async fn run(config: &Config) -> Result<()> {
    let addr = format!("{}:{}", config.server.listen, config.server.port);
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("Listening on {}", addr);

    let layout = parse_layout(&config.layout.position);
    tracing::info!("Layout: {:?}", layout);

    loop {
        let (stream, peer) = listener.accept().await?;
        tracing::info!("Client connected from {}", peer);
        tokio::spawn(handle_client(stream, layout));
    }
}

fn parse_layout(s: &str) -> LayoutPosition {
    match s {
        "right-left" => LayoutPosition::RightLeft,
        "top-bottom" => LayoutPosition::TopBottom,
        "bottom-top" => LayoutPosition::BottomTop,
        _ => LayoutPosition::LeftRight,
    }
}

async fn handle_client(stream: TcpStream, layout: LayoutPosition) -> Result<()> {
    let (mut reader, mut writer) = stream.into_split();

    let (client_screen, remaining_reader) = read_hello(reader).await?;
    reader = remaining_reader;

    let (screen_w, screen_h) = get_screen_size();

    let hello_ack = Message::HelloAck(HelloAckPayload {
        screen: ScreenInfo {
            width: screen_w as u16,
            height: screen_h as u16,
        },
        layout,
    });
    send_message(&mut writer, &hello_ack).await?;

    tracing::info!(
        "Handshake complete: server {}x{}, client {}x{}, layout: {:?}",
        screen_w, screen_h, client_screen.width, client_screen.height, layout
    );

    let (capture_tx, mut capture_rx) = mpsc::channel::<CaptureEvent>(4096);
    capture::start_capture(capture_tx)?;

    let (leave_tx, mut leave_rx) = mpsc::channel::<Edge>(4);

    let edge_detector = std::sync::Arc::new(tokio::sync::Mutex::new(
        EdgeDetector::new(screen_w as i32, screen_h as i32)
    ));

    let mut writer = writer;
    let mut last_edge: Option<Edge> = None;
    let mut last_edge_y: i32 = 0;
    let mut event_count: u64 = 0;
    let mut cooldown_until: Option<tokio::time::Instant> = None;
    let cooldown_duration = std::time::Duration::from_millis(300);
    let mut virtual_x: i32 = 0;
    let mut virtual_y: i32 = 0;

    let mut reader_buf = BytesMut::with_capacity(4096);
    let mut read_buf = [0u8; 4096];

    loop {
        tokio::select! {
            Some(event) = capture_rx.recv() => {
                event_count += 1;
                let target = edge_detector.lock().await.target;

                match target {
                    FocusTarget::Server => {
                        let is_mouse_move = matches!(&event.message, Message::MouseMove(_));
                        if is_mouse_move {
                            let cooled_down = cooldown_until.map_or(true, |t| tokio::time::Instant::now() >= t);
                            if cooled_down {
                                let edge = edge_detector.lock().await.check(event.abs_x, event.abs_y, &layout);
                                if let Some(edge) = edge {
                                    last_edge = Some(edge);
                                    last_edge_y = event.abs_y;
                                    crate::capture::set_block_mouse(true);
                                    cooldown_until = None;
                                    virtual_x = match edge {
                                        Edge::Right => 0,
                                        Edge::Left => client_screen.width as i32 - 1,
                                        Edge::Bottom => 0,
                                        Edge::Top => client_screen.height as i32 - 1,
                                    };
                                    virtual_y = match edge {
                                        Edge::Right | Edge::Left => {
                                            let mapped = event.abs_y as f64 / screen_h as f64 * client_screen.height as f64;
                                            mapped as i32
                                        }
                                        Edge::Bottom => 0,
                                        Edge::Top => 0,
                                    };
                                    tracing::info!(
                                        "Edge detected at ({}, {}): {:?}, switching to client",
                                        event.abs_x, event.abs_y, edge
                                    );
                                    let enter_msg = Message::EdgeEnter(EdgeEnterPayload {
                                        edge,
                                        position: event.abs_y as u16,
                                    });
                                    if let Err(e) = send_message(&mut writer, &enter_msg).await {
                                        tracing::error!("Failed to send EdgeEnter: {}", e);
                                    }
                                    continue;
                                }
                            }

                            if event_count % 2000 == 0 {
                                tracing::info!(
                                    "Server mode: {} events, cursor ({}, {})",
                                    event_count, event.abs_x, event.abs_y
                                );
                            }
                        }
                    }
                    FocusTarget::Client => {
                        let msg = match &event.message {
                            Message::MouseMove(_) => {
                                virtual_x += event.dx;
                                virtual_y += event.dy;
                                virtual_x = virtual_x.clamp(0, client_screen.width as i32 - 1);
                                virtual_y = virtual_y.clamp(0, client_screen.height as i32 - 1);
                                Message::MouseMove(MouseMovePayload {
                                    x: virtual_x,
                                    y: virtual_y,
                                })
                            }
                            other => other.clone(),
                        };
                        if let Err(e) = send_message(&mut writer, &msg).await {
                            tracing::error!("Failed to forward event: {}", e);
                            edge_detector.lock().await.return_to_server();
                            crate::capture::set_block_mouse(false);
                            break;
                        }

                        if let Some(edge) = last_edge {
                            pin_cursor_at_edge(edge, last_edge_y, screen_w as i32, screen_h as i32);
                        }

                        if event_count % 2000 == 0 {
                            tracing::info!("Client mode: {} events forwarded", event_count);
                        }
                    }
                }
            }
            Some(_edge) = leave_rx.recv() => {
                tracing::info!("EdgeLeave received from client, switching back to server");
                edge_detector.lock().await.return_to_server();
                last_edge = None;
                crate::capture::set_block_mouse(false);
                warp_cursor_inward(screen_w as i32, screen_h as i32);
                cooldown_until = Some(tokio::time::Instant::now() + cooldown_duration);
                let _ = send_message(&mut writer, &Message::EdgeLeave(EdgeLeavePayload {
                    edge: Edge::Left,
                })).await;
            }
            result = reader.readable() => {
                if let Ok(()) = result {
                    match reader.try_read(&mut read_buf) {
                        Ok(0) => {
                            tracing::info!("Client disconnected");
                            break;
                        }
                        Ok(n) => {
                            reader_buf.extend_from_slice(&read_buf[..n]);
                            while let Some(msg) = serialize::decode(&mut reader_buf)? {
                                if let Message::EdgeLeave(p) = msg {
                                    tracing::info!("Client sent EdgeLeave: edge={:?}", p.edge);
                                    let _ = leave_tx.send(p.edge).await;
                                }
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(e) => {
                            tracing::warn!("Client read error: {}", e);
                            break;
                        }
                    }
                }
            }
            else => {
                break;
            }
        }
    }

    tracing::info!("Client handler exiting");
    Ok(())
}

async fn read_hello(
    mut reader: tokio::net::tcp::OwnedReadHalf,
) -> Result<(ScreenInfo, tokio::net::tcp::OwnedReadHalf)> {
    let mut buf = BytesMut::with_capacity(4096);
    let mut read_buf = [0u8; 4096];

    loop {
        reader.readable().await?;
        match reader.try_read(&mut read_buf) {
            Ok(0) => {
                return Err(anyhow::anyhow!("Client disconnected before hello"));
            }
            Ok(n) => {
                buf.extend_from_slice(&read_buf[..n]);
                while let Some(msg) = serialize::decode(&mut buf)? {
                    if let Message::Hello(p) = msg {
                        tracing::info!("Received Hello from client: {}x{}", p.screen.width, p.screen.height);
                        return Ok((p.screen, reader));
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

async fn send_message(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    msg: &Message,
) -> Result<()> {
    let mut buf = BytesMut::new();
    serialize::encode(msg, &mut buf);
    writer.write_all(&buf).await?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn pin_cursor_at_edge(_edge: Edge, _y: i32, _screen_w: i32, _screen_h: i32) {}

#[cfg(target_os = "linux")]
fn warp_cursor_inward(_sw: i32, _sh: i32) {}

#[cfg(target_os = "windows")]
fn warp_cursor_inward(sw: i32, _sh: i32) {
    use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;
    unsafe {
        let _ = SetCursorPos(sw - 10, 0);
    }
}

#[cfg(target_os = "windows")]
fn pin_cursor_at_edge(edge: Edge, y: i32, screen_w: i32, screen_h: i32) {
    use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;
    let (x, y) = match edge {
        Edge::Right => (screen_w - 1, y.clamp(0, screen_h - 1)),
        Edge::Left => (0, y.clamp(0, screen_h - 1)),
        Edge::Bottom => (0, screen_h - 1),
        Edge::Top => (0, 0),
    };
    unsafe {
        let _ = SetCursorPos(x, y);
    }
}

#[cfg(target_os = "linux")]
fn get_screen_size() -> (u32, u32) {
    use x11::xlib::{XDefaultScreenOfDisplay, XOpenDisplay};
    unsafe {
        let display = XOpenDisplay(std::ptr::null());
        if display.is_null() {
            tracing::warn!("Cannot open X display, using default 1920x1080");
            return (1920, 1080);
        }
        let screen = XDefaultScreenOfDisplay(display);
        let width = (*screen).width as u32;
        let height = (*screen).height as u32;
        x11::xlib::XCloseDisplay(display);
        (width, height)
    }
}

#[cfg(target_os = "windows")]
fn get_screen_size() -> (u32, u32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN) as u32;
        let h = GetSystemMetrics(SM_CYSCREEN) as u32;
        if w > 0 && h > 0 {
            (w, h)
        } else {
            (1920, 1080)
        }
    }
}
