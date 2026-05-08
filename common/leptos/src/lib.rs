//! # Leptos (native UI port)
//!
//! Renderer-agnostic core. Each native target (cocoa, gtk, uikit)
//! provides its own `Renderer` impl in its `leptos_<platform>`
//! crate. View types are generic over `R: Renderer`.
//!
//! ## Phase 7 status
//!
//! Phase 7 has landed the scaffold: `Cargo.toml` rewritten to depend
//! only on the native-side crates (`renderer`, `leptos_macro`,
//! `reactive_graph`, plus utility crates), and `IntoView` has had its
//! `RenderHtml` bound dropped and is now generic over `R: Renderer`.
//!
//! Web-only modules (`mount`, `form`, `await_`, `nonce`, `subsecond`,
//! `attribute_interceptor`, `hydration/`, `from_form_data`) have been
//! deleted.
//!
//! Components that compile against the new core: `component`,
//! `into_view`, `provider`, `text_prop`, `logging`. Those are
//! `pub use`'d below.
//!
//! Components still on the Phase 8 punch list (require R-genericization
//! against the new generic `Render<R>` in their `Render` impls and
//! `IntoView<R>` bounds, plus a renderer-agnostic replacement for
//! `tachys::either::Either`): `Show`, `ShowLet`, `For`, `children`,
//! `error_boundary`, `suspense`, `transition`, `animated_show`,
//! `portal`. The source files for `Show`, `ShowLet`, `For`, `children`
//! remain on disk under their original names (no `mod` line here)
//! pending refactor; the others were deleted in Phase 7 and will be
//! ported back in from `/Users/seph/src/leptos-upstream/leptos/src/`
//! against the new shape.

#![cfg_attr(all(feature = "nightly", rustc_nightly), feature(fn_traits))]
#![cfg_attr(all(feature = "nightly", rustc_nightly), feature(unboxed_closures))]

extern crate self as leptos;

/// Re-exports the core types of the library.
pub mod prelude {
    pub use reactive_graph::prelude::*;
    pub use renderer::prelude::*;

    pub use crate::{
        children::*, component::*, control_flow::*, error::*, into_view::*,
        text_prop::*,
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

/// Control-flow components like `<Show>`, `<ShowLet>`, `<For>`.
pub mod control_flow {
    pub use crate::{for_loop::*, show::*, show_let::*};
}
mod for_loop;
mod show;
mod show_let;

/// Types for reactive string properties for components.
pub mod text_prop;

pub use leptos_macro::*;
#[doc(hidden)]
pub use typed_builder;
#[doc(hidden)]
pub use typed_builder_macro;

mod into_view;
pub use into_view::IntoView;

mod provider;

#[doc(inline)]
pub use either_of as either;
#[doc(inline)]
pub use reactive_graph as reactive;
#[doc(inline)]
pub use renderer;
#[doc(inline)]
pub use oco_ref as oco;

/// Provide and access data along the reactive graph, sharing data without directly passing arguments.
pub mod context {
    pub use crate::provider::*;
    pub use reactive_graph::owner::{provide_context, use_context};
}

/// Utilities for simple logging.
pub mod logging;

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

#[doc(hidden)]
pub use serde_json;
#[cfg(feature = "tracing")]
#[doc(hidden)]
pub use tracing;

#[doc(hidden)]
pub mod __reexports {
    pub use send_wrapper;
}
