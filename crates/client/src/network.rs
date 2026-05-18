use anyhow::Result;
use bytes::BytesMut;
use softkvm_common::Config;
use softkvm_protocol::message::*;
use softkvm_protocol::serialize;
use softkvm_protocol::Message;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use crate::inject::InputInjector;
use std::sync::Mutex;

static INJECTOR: once_cell::sync::Lazy<Mutex<InputInjector>> = once_cell::sync::Lazy::new(|| {
    Mutex::new(InputInjector::new().expect("Failed to create input injector"))
});

fn inject_mouse_move(dx: i16, dy: i16) {
    if let Ok(mut inj) = INJECTOR.lock() {
        let _ = inj.mouse_move(dx, dy);
    }
}

fn inject_mouse_button(button: MouseButtonId, state: ButtonState) {
    if let Ok(mut inj) = INJECTOR.lock() {
        let _ = inj.mouse_button(button, state);
    }
}

fn inject_mouse_scroll(delta: i16) {
    if let Ok(mut inj) = INJECTOR.lock() {
        let _ = inj.mouse_scroll(delta);
    }
}

fn inject_mouse_move_absolute(x: i32, y: i32, screen_w: i32, screen_h: i32) {
    if let Ok(mut inj) = INJECTOR.lock() {
        let _ = inj.mouse_move_absolute(x, y, screen_w, screen_h);
    }
}

fn inject_key(keycode: u16, pressed: bool) {
    if let Ok(mut inj) = INJECTOR.lock() {
        let _ = inj.key(keycode, pressed);
    }
}

pub async fn run(config: &Config) -> Result<()> {
    let addr = format!("{}:{}", config.client.host, config.client.port);
    tracing::info!("Connecting to {}", addr);

    let stream = TcpStream::connect(&addr).await?;
    tracing::info!("Connected to server");
    let (reader, mut writer) = stream.into_split();

    let (screen_w, screen_h) = get_screen_size();

    let hello = Message::Hello(HelloPayload {
        screen: ScreenInfo {
            width: screen_w as u16,
            height: screen_h as u16,
        },
    });
    send_message(&mut writer, &hello).await?;

    let writer = std::sync::Arc::new(tokio::sync::Mutex::new(writer));
    let mut active = false;
    let mut entry_edge: Option<Edge> = None;
    let mut moved_away = false;
    let edge_size: i32 = 5;
    let away_threshold: i32 = 30;

    let mut buf = BytesMut::with_capacity(4096);
    let mut read_buf = [0u8; 4096];
    let mut reader = reader;

    loop {
        let n = reader.readable().await?;
        match reader.try_read(&mut read_buf) {
            Ok(0) => {
                tracing::info!("Disconnected from server");
                return Ok(());
            }
            Ok(n) => {
                buf.extend_from_slice(&read_buf[..n]);
                while let Some(msg) = serialize::decode(&mut buf)? {
                    match msg {
                        Message::HelloAck(p) => {
                            tracing::info!(
                                "Server hello ack: {}x{}, layout: {:?}",
                                p.screen.width, p.screen.height, p.layout
                            );
                        }
                        Message::MouseMove(p) => {
                            if active {
                                inject_mouse_move(p.dx, p.dy);

                                if let Some(entry) = entry_edge {
                                    if let Some((cx, cy)) = get_cursor_pos() {
                                        if !moved_away {
                                            let far_enough = match entry {
                                                Edge::Right => cx >= away_threshold,
                                                Edge::Left => cx <= (screen_w as i32 - away_threshold),
                                                Edge::Bottom => cy >= away_threshold,
                                                Edge::Top => cy <= (screen_h as i32 - away_threshold),
                                            };
                                            if far_enough {
                                                moved_away = true;
                                            }
                                        }
                                        if moved_away {
                                            let should_leave = match entry {
                                                Edge::Right => cx <= edge_size,
                                                Edge::Left => cx >= (screen_w as i32 - edge_size),
                                                Edge::Bottom => cy <= edge_size,
                                                Edge::Top => cy >= (screen_h as i32 - edge_size),
                                            };
                                            if should_leave {
                                                tracing::info!(
                                                    "Cursor at ({}, {}) hit return edge, sending EdgeLeave",
                                                    cx, cy
                                                );
                                                active = false;
                                                entry_edge = None;
                                                moved_away = false;
                                                let w = writer.clone();
                                                tokio::spawn(async move {
                                                    let mut w = w.lock().await;
                                                    let _ = send_message(&mut w, &Message::EdgeLeave(EdgeLeavePayload {
                                                        edge: Edge::Left,
                                                    })).await;
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Message::MouseButton(p) => {
                            if active {
                                inject_mouse_button(p.button, p.state);
                            }
                        }
                        Message::MouseScroll(p) => {
                            if active {
                                inject_mouse_scroll(p.delta);
                            }
                        }
                        Message::KeyDown(p) => {
                            if active {
                                inject_key(p.keycode, true);
                            }
                        }
                        Message::KeyUp(p) => {
                            if active {
                                inject_key(p.keycode, false);
                            }
                        }
                        Message::EdgeEnter(p) => {
                            tracing::info!("EdgeEnter: edge={:?}, position={}", p.edge, p.position);
                            active = true;
                            entry_edge = Some(p.edge);
                            moved_away = false;
                            let sw = screen_w as i32;
                            let sh = screen_h as i32;
                            match p.edge {
                                Edge::Left => {
                                    inject_mouse_move_absolute(sw - 1, p.position as i32, sw, sh);
                                }
                                Edge::Right => {
                                    inject_mouse_move_absolute(0, p.position as i32, sw, sh);
                                }
                                Edge::Top => {
                                    inject_mouse_move_absolute(p.position as i32, sh - 1, sw, sh);
                                }
                                Edge::Bottom => {
                                    inject_mouse_move_absolute(p.position as i32, 0, sw, sh);
                                }
                            }
                        }
                        Message::EdgeLeave(_) => {
                            tracing::info!("EdgeLeave received");
                        }
                        Message::Clipboard(p) => {
                            if let Err(e) = crate::clipboard::set_clipboard(&p.data) {
                                tracing::warn!("Failed to set clipboard: {}", e);
                            }
                        }
                        Message::Heartbeat => {}
                        _ => {
                            tracing::debug!("Received: {:?}", msg);
                        }
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
fn get_cursor_pos() -> Option<(i32, i32)> {
    use x11::xlib::{XOpenDisplay, XQueryPointer, XDefaultRootWindow};
    unsafe {
        let display = XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return None;
        }
        let root = XDefaultRootWindow(display);
        let mut root_x = 0i32;
        let mut root_y = 0i32;
        let mut win_x = 0i32;
        let mut win_y = 0i32;
        let mut root_return = 0u64;
        let mut child_return = 0u64;
        let mut mask_return = 0u32;
        XQueryPointer(
            display,
            root,
            &mut root_return,
            &mut child_return,
            &mut root_x,
            &mut root_y,
            &mut win_x,
            &mut win_y,
            &mut mask_return,
        );
        x11::xlib::XCloseDisplay(display);
        Some((root_x, root_y))
    }
}

#[cfg(target_os = "windows")]
fn get_cursor_pos() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    unsafe {
        let mut point = POINT::default();
        GetCursorPos(&mut point).ok()?;
        Some((point.x, point.y))
    }
}

#[cfg(target_os = "linux")]
fn get_screen_size() -> (u32, u32) {
    use x11::xlib::{XDefaultScreenOfDisplay, XOpenDisplay};
    unsafe {
        let display = XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return (1920, 1080);
        }
        let screen = XDefaultScreenOfDisplay(display);
        let w = (*screen).width as u32;
        let h = (*screen).height as u32;
        x11::xlib::XCloseDisplay(display);
        (w, h)
    }
}

#[cfg(target_os = "windows")]
fn get_screen_size() -> (u32, u32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN) as u32;
        let h = GetSystemMetrics(SM_CYSCREEN) as u32;
        if w > 0 && h > 0 { (w, h) } else { (1920, 1080) }
    }
}
