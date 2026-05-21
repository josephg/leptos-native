//! GTK transport for the Chrome DevTools Protocol server.
//!
//! The renderer-agnostic CDP logic lives in `leptos_devtools`; this module
//! supplies the GTK-specific socket integration. We listen on a TCP port
//! with a `gio::SocketListener` (whose async accept + read/write futures
//! are integrated with the glib main context), and feed each accepted
//! connection to `leptos_devtools::serve_connection` on the glib main-loop
//! executor. Because that future is polled on the main thread, the CDP
//! dispatcher reaches the thread-local layout tree synchronously.
//!
//! Enabled by the `devtools` cargo feature; started at runtime only when
//! `LEPTOS_DEVTOOLS` is set (to a port number, or any value for the
//! default port). See [`start_from_env`].

use crate::dom::layout::{schedule_relayout_for, GtkBackend};
use futures::{AsyncRead, AsyncWrite};
use gtk4::prelude::*;
use renderer::NodeId;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};
use crate::dom::highlight;
use renderer::LayoutBackend;

const DEFAULT_PORT: u16 = 9223;

/// Displayable attributes for a node, read from its GTK widget — shown
/// next to the tag in the Elements tree (e.g. `button title="Reset"`).
fn node_attributes(id: NodeId) -> Vec<(String, String)> {
    let Some(w) = GtkBackend::view(id) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut push = |k: &str, v: String| {
        if !v.is_empty() {
            out.push((k.to_string(), v));
        }
    };

    if let Some(b) = w.downcast_ref::<gtk4::Button>() {
        push("title", b.label().map(|s| s.to_string()).unwrap_or_default());
    } else if let Some(l) = w.downcast_ref::<gtk4::Label>() {
        push("value", l.label().to_string());
    } else if let Some(e) = w.downcast_ref::<gtk4::Entry>() {
        push("value", e.text().to_string());
    } else if let Some(c) = w.downcast_ref::<gtk4::CheckButton>() {
        out.push(("checked".into(), c.is_active().to_string()));
    } else if let Some(s) = w.downcast_ref::<gtk4::Scale>() {
        out.push(("value".into(), format!("{}", s.value())));
    } else if let Some(d) = w.downcast_ref::<gtk4::DropDown>() {
        out.push(("selected".into(), d.selected().to_string()));
    }
    out
}

/// Start the devtools server if `LEPTOS_DEVTOOLS` is set in the
/// environment. The value may be a port number (e.g. `9223`); any other
/// non-empty value uses the default port. A no-op if the var is unset.
///
/// Call this once, from inside the GTK `activate` handler (so the
/// spawner and main loop are live).
pub fn start_from_env() {
    let Ok(val) = std::env::var("LEPTOS_DEVTOOLS") else {
        return;
    };
    let port = val.trim().parse::<u16>().unwrap_or(DEFAULT_PORT);
    start(port);
}

/// Start the devtools server on `127.0.0.1:port`.
pub fn start(port: u16) {
    let listener = gio::SocketListener::new();
    if let Err(e) = listener.add_inet_port(port, glib::Object::NONE) {
        eprintln!("devtools: failed to listen on port {port}: {e}");
        return;
    }
    let host = format!("127.0.0.1:{port}");
    eprintln!("devtools: listening on http://{host} (open with Chrome DevTools)");

    any_spawner::Executor::spawn_local(async move {
        loop {
            match listener.accept_future().await {
                Ok((conn, _src)) => match conn.into_async_read_write() {
                    Ok(stream) => {
                        let hooks = leptos_devtools::Hooks {
                            schedule_relayout: Rc::new(schedule_relayout_for),
                            set_highlight: Rc::new(highlight::set_highlight),
                            node_attributes: Rc::new(node_attributes),
                            set_inspect_mode: Rc::new(highlight::set_inspect_mode),
                        };
                        any_spawner::Executor::spawn_local(
                            leptos_devtools::serve_connection::<_, GtkBackend>(
                                MainThreadStream(stream),
                                host.clone(),
                                hooks,
                            ),
                        );
                    }
                    Err(_) => eprintln!("devtools: connection is not pollable"),
                },
                Err(e) => {
                    eprintln!("devtools: accept failed, stopping listener: {e}");
                    break;
                }
            }
        }
    });
}

type GioStream = gio::IOStreamAsyncReadWrite<gio::SocketConnection>;

/// A glib-backed socket stream that is only ever polled on the main
/// thread (via `spawn_local`). hyper's upgrade machinery requires the
/// transport to be `Send`, but the value never actually crosses threads;
/// the `unsafe impl Send` is sound under that main-thread-only invariant.
struct MainThreadStream(GioStream);

// SAFETY: the stream is created and polled exclusively on the GTK main
// thread; it is never moved to or accessed from another thread. The
// `Send` bound exists only to satisfy hyper's `Upgraded` type.
unsafe impl Send for MainThreadStream {}

impl AsyncRead for MainThreadStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut Pin::get_mut(self).0).poll_read(cx, buf)
    }
}

impl AsyncWrite for MainThreadStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut Pin::get_mut(self).0).poll_write(cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut Pin::get_mut(self).0).poll_flush(cx)
    }
    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut Pin::get_mut(self).0).poll_close(cx)
    }
}
