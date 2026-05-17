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

    let hello = Message::Hello(HelloPayload {
        screen: ScreenInfo {
            width: 1920,
            height: 1080,
        },
    });
    send_message(&mut writer, &hello).await?;

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
                    handle_message(&msg);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn handle_message(msg: &Message) {
    match msg {
        Message::HelloAck(p) => {
            tracing::info!(
                "Server hello ack: {}x{}, layout: {:?}",
                p.screen.width,
                p.screen.height,
                p.layout
            );
        }
        Message::MouseMove(p) => {
            inject_mouse_move(p.dx, p.dy);
        }
        Message::MouseButton(p) => {
            inject_mouse_button(p.button, p.state);
        }
        Message::MouseScroll(p) => {
            inject_mouse_scroll(p.delta);
        }
        Message::KeyDown(p) => {
            inject_key(p.keycode, true);
        }
        Message::KeyUp(p) => {
            inject_key(p.keycode, false);
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

async fn send_message(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    msg: &Message,
) -> Result<()> {
    let mut buf = BytesMut::new();
    serialize::encode(msg, &mut buf);
    writer.write_all(&buf).await?;
    Ok(())
}
