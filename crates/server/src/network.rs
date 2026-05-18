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
    let (reader, mut writer) = stream.into_split();

    let (hello_buf, client_screen) = read_hello(reader).await?;
    let _ = hello_buf;

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

    let mut edge_detector = EdgeDetector::new(screen_w as i32, screen_h as i32);
    let mut writer = writer;

    while let Some(event) = capture_rx.recv().await {
        match edge_detector.target {
            FocusTarget::Server => {
                if let Message::MouseMove(_) = &event.message {
                    let edge = edge_detector.check(event.abs_x, event.abs_y, &layout);
                    if let Some(edge) = edge {
                        tracing::info!("Edge detected: {:?}, switching to client", edge);
                        let enter_msg = Message::EdgeEnter(EdgeEnterPayload {
                            edge,
                            position: event.abs_y as u16,
                        });
                        let _ = send_message(&mut writer, &enter_msg).await;
                        continue;
                    }
                }
            }
            FocusTarget::Client => {
                let _ = send_message(&mut writer, &event.message).await;
            }
        }
    }

    tracing::info!("Capture loop ended, client handler exiting");
    Ok(())
}

async fn read_hello(
    reader: tokio::net::tcp::OwnedReadHalf,
) -> Result<(BytesMut, ScreenInfo)> {
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
                        return Ok((buf, p.screen));
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
    use windows::Win32::Graphics::Gdi::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
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
