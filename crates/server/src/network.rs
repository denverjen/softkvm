use anyhow::Result;
use bytes::BytesMut;
use softkvm_common::Config;
use softkvm_protocol::serialize;
use softkvm_protocol::Message;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

const MAX_FRAME_SIZE: usize = 1024 * 1024;

pub async fn run(config: &Config) -> Result<()> {
    let addr = format!("{}:{}", config.server.listen, config.server.port);
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("Listening on {}", addr);

    loop {
        let (stream, peer) = listener.accept().await?;
        tracing::info!("Client connected from {}", peer);
        tokio::spawn(handle_client(stream));
    }
}

async fn handle_client(stream: TcpStream) -> Result<()> {
    let (reader, _writer) = stream.into_split();
    let mut buf = BytesMut::with_capacity(4096);
    let mut read_buf = [0u8; 4096];
    let mut reader = reader;

    loop {
        let n = reader.readable().await?;
        match reader.try_read(&mut read_buf) {
            Ok(0) => {
                tracing::info!("Client disconnected");
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
        Message::Hello(p) => {
            tracing::info!(
                "Client hello: {}x{}",
                p.screen.width,
                p.screen.height
            );
        }
        Message::EdgeLeave(p) => {
            tracing::info!("Client edge leave: {:?}", p.edge);
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
    use tokio::io::AsyncWriteExt;
    writer.write_all(&buf).await?;
    Ok(())
}
