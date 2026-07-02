//! Port-agnostic CDP transport: HTTP discovery + a WebSocket session,
//! served over a single already-connected stream.
//!
//! The port owns the listener and the native-loop socket integration
//! (gio on GTK, dispatch on cocoa/iOS); it hands each accepted connection
//! — anything implementing `futures` [`AsyncRead`]/[`AsyncWrite`] — to
//! [`serve_connection`]. We bridge that to tokio's I/O traits with the
//! tiny [`Adapter`] (the exact pattern from yawc's smol example), wrap it
//! in [`TokioIo`], and run hyper's HTTP/1 server with upgrades. The whole
//! future is polled by the port's main-loop executor, so the CDP
//! dispatcher touches the thread-local layout tree directly.

use crate::session::Session;
use crate::Hooks;
use futures::{SinkExt, StreamExt};
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use hyper_util::rt::TokioIo;
use leptos_native::renderer::Backend;
use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};
use yawc::{Frame, OpCode, WebSocket};

/// Bridges a `futures` async stream to tokio's `AsyncRead`/`AsyncWrite`,
/// which is all `hyper`/`yawc` require of the underlying transport.
struct Adapter<S>(S);

impl<S: futures::AsyncRead + Unpin> tokio::io::AsyncRead for Adapter<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let unfilled = buf.initialize_unfilled();
        match Pin::new(&mut self.0).poll_read(cx, unfilled) {
            Poll::Ready(Ok(n)) => {
                buf.advance(n);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S: futures::AsyncWrite + Unpin> tokio::io::AsyncWrite for Adapter<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_close(cx)
    }
}

fn json_response(body: serde_json::Value) -> Response<Full<Bytes>> {
    Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

fn version_json(host: &str) -> serde_json::Value {
    serde_json::json!({
        "Browser": "leptos-native/0.1",
        "Protocol-Version": "1.3",
        "User-Agent": "leptos-native",
        "V8-Version": "0",
        "WebKit-Version": "0",
        "webSocketDebuggerUrl": format!("ws://{host}/devtools/browser/leptos"),
    })
}

fn targets_json(host: &str) -> serde_json::Value {
    serde_json::json!([{
        "description": "leptos-native app",
        "id": "leptos-page",
        "title": "leptos-native",
        "type": "page",
        "url": "leptos://app",
        "webSocketDebuggerUrl": format!("ws://{host}/devtools/page/leptos-page"),
        "devtoolsFrontendUrl": format!(
            "devtools://devtools/bundled/inspector.html?ws={host}/devtools/page/leptos-page"
        ),
    }])
}

/// Serve CDP over one connection: HTTP discovery routes plus a WebSocket
/// upgrade that runs the [`Session`] dispatcher. Returns when the
/// connection closes.
pub async fn serve_connection<S, B>(stream: S, host: String, hooks: Hooks)
where
    // `Send` is required only by hyper's upgrade machinery
    // (`hyper::upgrade::Upgraded` is `Send`); the connection future itself
    // is still `!Send` and is driven by the port's main-loop executor. A
    // main-thread-only socket type can satisfy this with a documented
    // `unsafe impl Send` wrapper.
    S: futures::AsyncRead + futures::AsyncWrite + Unpin + Send + 'static,
    B: Backend,
{
    let io = TokioIo::new(Adapter(stream));

    let service = service_fn(move |req: Request<Incoming>| {
        let host = host.clone();
        let hooks = hooks.clone();
        async move {
            let path = req.uri().path();
            if req.method() == Method::GET && path == "/json/version" {
                return Ok::<_, Infallible>(json_response(version_json(&host)));
            }
            if req.method() == Method::GET && (path == "/json" || path == "/json/list") {
                return Ok(json_response(targets_json(&host)));
            }

            // Anything else is treated as a WebSocket upgrade.
            match WebSocket::upgrade(req) {
                Ok((response, upgrade_fut)) => {
                    any_spawner::Executor::spawn_local(async move {
                        match upgrade_fut.await {
                            Ok(ws) => run_session::<_, B>(ws, hooks).await,
                            Err(e) => eprintln!("devtools: ws upgrade failed: {e}"),
                        }
                    });
                    Ok(response.map(|_| Full::new(Bytes::new())))
                }
                Err(e) => {
                    eprintln!("devtools: not a websocket request: {e}");
                    Ok(Response::builder()
                        .status(400)
                        .body(Full::new(Bytes::new()))
                        .unwrap())
                }
            }
        }
    });

    if let Err(e) = hyper::server::conn::http1::Builder::new()
        .serve_connection(io, service)
        .with_upgrades()
        .await
    {
        eprintln!("devtools: connection closed: {e}");
    }
}

async fn run_session<Ws, B>(ws: Ws, hooks: Hooks)
where
    Ws: futures::Stream<Item = Frame> + futures::Sink<Frame> + Unpin,
    B: Backend,
{
    use futures::FutureExt;

    let mut session = Session::<B>::new(hooks);
    // Split so we can read incoming frames and write backend-pushed
    // events (inspect-mode) concurrently without borrow conflicts.
    let (mut tx, mut rx) = ws.split();
    let mut outgoing = crate::events::register();

    loop {
        futures::select! {
            frame = rx.next().fuse() => {
                let Some(frame) = frame else { return };
                match frame.opcode() {
                    OpCode::Text | OpCode::Binary => {
                        let payload = frame.payload().clone();
                        let Ok(text) = std::str::from_utf8(&payload) else { continue };
                        for reply in session.dispatch(text) {
                            if tx.send(Frame::text(reply)).await.is_err() {
                                return;
                            }
                        }
                    }
                    OpCode::Close => return,
                    _ => {} // ping/pong handled by yawc
                }
            }
            event = outgoing.next().fuse() => {
                let Some(event) = event else { continue };
                if tx.send(Frame::text(event)).await.is_err() {
                    return;
                }
            }
        }
    }
}
