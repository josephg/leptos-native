//! # Leptos — renderer-agnostic core (native UI fork)
//!
//! This crate is the platform-independent core of `leptos-mac`, a
//! native-only fork of [Leptos](https://leptos.dev). It defines the
//! [`IntoView<R>`] trait, the component-system glue, control-flow
//! components, and the `<ErrorBoundary>` machinery — but it does
//! **not** render anything by itself. Rendering happens through a
//! platform-specific [`Renderer`](renderer::renderer::Renderer) impl provided by one of
//! the sibling crates:
//!
//! | Platform | Crate         | Backend  |
//! |----------|---------------|----------|
//! | macOS    | `leptos_cocoa` | AppKit   |
//! | iOS      | `leptos_uikit` | UIKit    |
//! | Linux    | `leptos_gtk`   | GTK4 *(in progress)* |
//!
//! End-user code depends on the platform crate as `leptos = { package
//! = "leptos_cocoa" | "leptos_uikit" | … }`. The platform crate
//! re-exports everything here under `::leptos_native::*`, plus its own
//! element builders (`button`, `vstack`, `<text_field>`, etc.) and
//! a `Dom` unit type that is the `Renderer` for that target.
//!
//! ## What's in this crate
//!
//! - [`IntoView<R>`] — every type that can be rendered. Generic over
//!   `R: Renderer` so the same view trees work across platforms; each
//!   platform crate provides a non-generic specialization
//!   (`pub trait IntoView: leptos_native::IntoView<Dom>`) so user code writes
//!   `impl IntoView` without the `<R>` parameter.
//! - [`children`] — typed children props (`TypedChildren<T, R>`,
//!   `TypedChildrenFn<T, R>`, `TypedChildrenMut<T, R>`) plus the
//!   erased `ChildrenFn<R>` (built on `AnyView<R>`) for slot-style
//!   patterns where children vary per call-site. See "Native vs
//!   upstream" below.
//! - [`component`] — the `Props` / `ComponentConstructor` plumbing
//!   the `#[component]` proc-macro emits against.
//! - [`control_flow`] — `<Show>`, `<ShowLet>`, `<For>`. Branching
//!   components ported against the new `Render<R>` shape. `<For>` is
//!   keyed via the `Keyed` adapter in `common/renderer`.
//! - [`error_boundary`] — `<ErrorBoundary>` + the `Errors` map.
//!   Catches `Result::Err` thrown by descendant `Result<T, E>:
//!   Render<R>` impls via the `throw_error` hook system.
//! - [`text_prop`] — `TextProp` / `OptionTextPropExt` for component
//!   props that take "string or signal-of-string".
//! - [`context`] — `provide_context` / `use_context` re-exports +
//!   the `<Provider>` component (per-subtree context override).
//! - [`logging`] — `log!`, `warn!`, `error!`, `debug_warn!` macros.
//!   Plain `println!`/`eprintln!` on native (no `web_sys::console`).
//!
//! ## A minimum viable example
//!
//! ```ignore
//! // Cargo.toml: leptos = { package = "leptos_cocoa", path = "..." }
//! use leptos_native::prelude::*;
//!
//! #[component]
//! fn Counter(initial: i32) -> impl IntoView {
//!     let count = RwSignal::new(initial);
//!     view! {
//!         <vstack padding=16.0 gap=12.0>
//!             <label>{move || format!("Count: {}", count.get())}</label>
//!             <hstack gap=8.0>
//!                 <button on:click=move |_| count.update(|n| *n -= 1)>"-1"</button>
//!                 <button on:click=move |_| count.set(0)>"Reset"</button>
//!                 <button on:click=move |_| count.update(|n| *n += 1)>"+1"</button>
//!             </hstack>
//!         </vstack>
//!     }
//! }
//!
//! fn main() {
//!     mount_to_window("Counter", (320.0, 200.0), || {
//!         view! { <Counter initial=0 /> }
//!     });
//! }
//! ```
//!
//! ## Native vs upstream
//!
//! This is **not a drop-in** for upstream Leptos. The fork removes
//! everything web/SSR/hydration-specific because the native ports
//! have no use for it. Notable removals:
//!
//! - **No `RenderHtml`** trait. `IntoView<R>` only requires
//!   `Render<R> + AddAnyAttr<R> + Send`. Native has no SSR step.
//! - **`AnyView<R>` is used sparingly.** Each binary has exactly
//!   one renderer; concrete view types pass through the component
//!   graph unmolested whenever possible (via `TypedChildren<C, R>`
//!   with a generic `C` parameter). For the cases where erasure is
//!   genuinely useful — slot children that vary per call-site,
//!   `<Show fallback>` with mismatched branch types —
//!   `renderer::view::AnyView<R>` and the per-port aliases
//!   `AnyView = AnyView<Dom>` are available, plus `ChildrenFn<R>`
//!   for erased children. See `cocoa/examples/slots`.
//! - **No `Suspense` / `Resource` / `Action::server_action`.** No
//!   server functions, so no async-data-bound view-rendering story.
//!   `task::spawn` exists for fire-and-forget futures but doesn't
//!   integrate with view rendering.
//! - **`<Transition>` works** for the async-render case (wrap one
//!   or more `Suspend`s; each shows a placeholder until its future
//!   resolves). Shared cross-Suspend "loading" coordination isn't
//!   wired yet. **`<AnimatedShow>`** is deferred — it needs
//!   platform animation integration (CoreAnimation on macOS / iOS,
//!   GTK transitions on Linux).
//!
//! See the platform crate's docs (`leptos_cocoa`, `leptos_uikit`)
//! for what each adds on top.

#![cfg_attr(all(feature = "nightly", rustc_nightly), feature(fn_traits))]
#![cfg_attr(all(feature = "nightly", rustc_nightly), feature(unboxed_closures))]

extern crate self as leptos_native;

/// Identity trait the `leptos_macro` view!{} expansion emits as
/// `::leptos_native::prelude::IntoAttributeValue::into_attribute_value(...)`
/// around attribute values. Upstream this normalised values into a
/// SSR-friendly `AttributeValue` shape; on native the value is
/// already the right type so the trait is a no-op identity.
pub trait IntoAttributeValue {
    type Output;
    fn into_attribute_value(self) -> Self::Output;
}

impl<T> IntoAttributeValue for T {
    type Output = T;
    fn into_attribute_value(self) -> Self { self }
}

pub mod renderer;

/// Re-exports the core types of the library.
pub mod prelude {
    pub use reactive_graph::prelude::*;
    pub use crate::renderer::prelude::*;

    pub use crate::{
        children::*, component::*, control_flow::*, error::*, into_view::*,
        local_resource::LocalResource, suspend::Suspend, text_prop::*,
        IntoAttributeValue,
    };

    pub use leptos_macro::*;
    pub use oco_ref::*;
    pub use reactive_graph::{
        actions::*,
        computed::*,
        effect::*,
        graph::untrack,
        owner::*,
        signal::*,
        wrappers::{read::*, write::*},
    };
}

/// A standard way to wrap functions and closures to pass them to components.
pub use reactive_graph::callback;

#[doc(hidden)]
/// Traits used to implement component constructors.
pub mod component;

/// Types that can be passed as the `children` prop of a component.
pub mod children;

/// `<ErrorBoundary>` component + the `Errors` map type.
pub mod error_boundary;

/// Tools for handling errors.
pub mod error {
    pub use crate::error_boundary::*;
    pub use throw_error::*;
}

/// Control-flow components like `<Show>`, `<ShowLet>`, `<For>`,
/// `<Switch>` / `<Match>`, `<Transition>`.
pub mod control_flow {
    pub use crate::{
        for_loop::*, show::*, show_let::*, switch::*, transition::*,
    };
}
mod for_loop;
mod show;
mod show_let;
mod switch;
mod transition;

/// `LocalResource<T>` — reactive async-derived value with no
/// `Send` bound on the closure. Pair with [`suspend::Suspend`].
pub mod local_resource;

/// `Suspend<F>` — render a future as a view; placeholder until
/// resolved, then the future's output mounted in place.
pub mod suspend;

/// Types for reactive string properties for components.
pub mod text_prop;

pub use leptos_macro::*;
#[doc(hidden)]
pub use typed_builder;
#[doc(hidden)]
pub use typed_builder_macro;

mod into_view;
pub use into_view::{IntoView, View};

mod provider;

#[doc(inline)]
pub use either_of as either;
#[doc(inline)]
pub use reactive_graph as reactive;
#[doc(inline)]
pub use oco_ref as oco;

/// Provide and access data along the reactive graph, sharing data without directly passing arguments.
pub mod context {
    pub use crate::provider::*;
    pub use reactive_graph::owner::{provide_context, use_context};
}

/// Utilities for simple logging.
pub mod logging;

pub mod node_ref;

/// Utilities for working with asynchronous tasks.
pub mod task {
    use any_spawner::Executor;
    use reactive_graph::computed::ScopedFuture;
    use std::future::Future;

    /// Spawns a thread-safe [`Future`].
    #[track_caller]
    #[inline(always)]
    pub fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
        let fut = ScopedFuture::new(fut);
        Executor::spawn(fut);
    }

    /// Spawns a [`Future`] that cannot be sent across threads.
    #[track_caller]
    #[inline(always)]
    pub fn spawn_local(fut: impl Future<Output = ()> + 'static) {
        Executor::spawn_local(fut)
    }

    /// Waits until the next "tick" of the current async executor.
    pub async fn tick() {
        Executor::tick().await
    }

    pub use reactive_graph::{
        spawn_local_scoped, spawn_local_scoped_with_cancellation,
    };
}

#[cfg(feature = "tracing")]
#[doc(hidden)]
pub use tracing;

#[doc(hidden)]
pub mod __reexports {
    pub use send_wrapper;
}
