//! Minimal SSE server for Anos events.
//!
//! Exposes:
//! - GET /health  -> plain OK
//! - GET /events  -> text/event-stream heartbeats

use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{interval, Duration};

pub async fn start(addr: String) -> Result<()> {
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("📡 SSE server listening on http://{}/events", addr);

    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                continue;
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/");

                if path == "/health" {
                    let _ = stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nOK")
                        .await;
                    return;
                }

                if path != "/events" {
                    let _ = stream
                        .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found")
                        .await;
                    return;
                }

                let headers = concat!(
                    "HTTP/1.1 200 OK\r\n",
                    "Content-Type: text/event-stream\r\n",
                    "Cache-Control: no-cache\r\n",
                    "Connection: keep-alive\r\n",
                    "Access-Control-Allow-Origin: *\r\n",
                    "\r\n"
                );
                if stream.write_all(headers.as_bytes()).await.is_err() {
                    return;
                }

                let hello = format!(
                    "event: start\ndata: {{\"version\":\"{}\",\"message\":\"Anos SSE connected\"}}\n\n",
                    env!("CARGO_PKG_VERSION")
                );
                if stream.write_all(hello.as_bytes()).await.is_err() {
                    return;
                }

                let mut tick = interval(Duration::from_secs(15));
                loop {
                    tick.tick().await;
                    let data = format!(
                        "event: heartbeat\ndata: {{\"ts\":\"{}\"}}\n\n",
                        chrono::Utc::now().to_rfc3339()
                    );
                    if stream.write_all(data.as_bytes()).await.is_err() {
                        break;
                    }
                }
            });
        }
    });

    Ok(())
}
