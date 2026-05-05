#[cfg(not(leptos_native))]
use self::attribute::Attribute;
#[cfg(not(leptos_native))]
use crate::{
    hydration::Cursor,
    no_attrs,
    prelude::{AddAnyAttr, Mountable},
    renderer::{
        dom::{Element, Node},
        CastFrom, Rndr,
    },
    view::{Position, PositionState, Render, RenderHtml},
};
#[cfg(not(leptos_native))]
use attribute::any_attribute::AnyAttribute;
#[cfg(not(leptos_native))]
use std::borrow::Cow;

/// Diagnostic message shared by event, directive, and property `.expect()` calls.
///
/// When the `ssr` feature is active, tachys skips creating client-side values
/// (event handlers, directives, properties) to avoid `SendWrapper` cross-thread
/// panics on multithreaded servers. If these `.expect()` calls fire, it means
/// the `ssr` feature was activated unintentionally via Cargo feature
/// unification in a client-side (CSR or hydrate) build.
///
/// Only referenced from the web-only event/directive/property modules.
pub(crate) const FEATURE_CONFLICT_DIAGNOSTIC: &str =
    "Value is None because the `ssr` feature is active. When `ssr` is \
     enabled, tachys skips creating client-side values (event handlers, \
     directives, properties) to avoid cross-thread panics on multithreaded \
     servers. If you are building a client-side (CSR or hydrate) target, this \
     means the `ssr` feature is being activated unintentionally via Cargo \
     feature unification; another dependency in your workspace is enabling \
     it. Run `cargo tree -e features -i tachys` to identify the source.";

/// Types for HTML attributes.
pub mod attribute;
/// Types for manipulating the `class` attribute and `classList`.
#[cfg(not(leptos_native))]
pub mod class;
/// Types for creating user-defined attributes with custom behavior (directives).
pub mod directive;
/// Types for HTML elements (web only — Cocoa elements live in their own
/// module, defined in Stage 5).
#[cfg(not(leptos_native))]
pub mod element;
/// On macOS native, `tachys::html::element` is a thin facade
/// re-exporting the Cocoa builders, so that `view!{}` macro
/// emissions like `::leptos::tachys::html::element::button()`
/// resolve correctly. Requires the `native-ui` and `reactive_graph`
/// features.
#[cfg(all(
    target_os = "macos",
    leptos_native,
    feature = "reactive_graph"
))]
pub mod element_macos;
#[cfg(all(
    target_os = "macos",
    leptos_native,
    feature = "reactive_graph"
))]
pub use element_macos as element;
/// On iOS native, same role — re-exports the UIKit builders at the
/// path the macro expects.
#[cfg(all(
    target_os = "ios",
    leptos_native,
    feature = "reactive_graph"
))]
pub mod element_ios;
#[cfg(all(
    target_os = "ios",
    leptos_native,
    feature = "reactive_graph"
))]
pub use element_ios as element;
/// On Linux native, same role — re-exports the GTK builders at the
/// path the macro expects.
#[cfg(all(
    target_os = "linux",
    leptos_native,
    feature = "reactive_graph"
))]
pub mod element_gtk;
#[cfg(all(
    target_os = "linux",
    leptos_native,
    feature = "reactive_graph"
))]
pub use element_gtk as element;

/// Types for DOM events.
#[cfg(not(leptos_native))]
pub mod event;
/// On macOS native, `tachys::html::event` is a thin facade providing
/// event descriptors and the `on(event, handler)` wrapper that maps
/// to our Cocoa target/action infrastructure. Requires the
/// `native-ui` and `reactive_graph` features.
#[cfg(all(
    target_os = "macos",
    leptos_native,
    feature = "reactive_graph"
))]
pub mod event_macos;
#[cfg(all(
    target_os = "macos",
    leptos_native,
    feature = "reactive_graph"
))]
pub use event_macos as event;
/// On iOS native, same role — event descriptors and the
/// `on(event, handler)` wrapper mapping to UIKit actions.
#[cfg(all(
    target_os = "ios",
    leptos_native,
    feature = "reactive_graph"
))]
pub mod event_ios;
#[cfg(all(
    target_os = "ios",
    leptos_native,
    feature = "reactive_graph"
))]
pub use event_ios as event;
/// On Linux native, same role — event descriptors and the
/// `on(event, handler)` wrapper mapping to GTK signals.
#[cfg(all(
    target_os = "linux",
    leptos_native,
    feature = "reactive_graph"
))]
pub mod event_gtk;
#[cfg(all(
    target_os = "linux",
    leptos_native,
    feature = "reactive_graph"
))]
pub use event_gtk as event;
/// Types for adding interactive islands to inert HTML pages.
#[cfg(not(leptos_native))]
pub mod islands;
/// Types for accessing a reference to an HTML element.
#[cfg(not(leptos_native))]
pub mod node_ref;
/// Types for DOM properties.
#[cfg(not(leptos_native))]
pub mod property;
/// Types for the `style` attribute and individual style manipulation.
#[cfg(not(leptos_native))]
pub mod style;

/// A `<!DOCTYPE>` declaration. Web-only — disabled on native targets
/// since the renderer has no concept of inert HTML or doctypes.
#[cfg(not(leptos_native))]
pub struct Doctype {
    value: &'static str,
}

/// Creates a `<!DOCTYPE>`.
#[cfg(not(leptos_native))]
pub fn doctype(value: &'static str) -> Doctype {
    Doctype { value }
}

#[cfg(not(leptos_native))]
impl Render for Doctype {
    type State = ();

    fn build(self) -> Self::State {}

    fn rebuild(self, _state: &mut Self::State) {}
}

#[cfg(not(leptos_native))]
no_attrs!(Doctype);

#[cfg(not(leptos_native))]
impl RenderHtml for Doctype {
    type AsyncOutput = Self;
    type Owned = Self;

    const MIN_LENGTH: usize = "<!DOCTYPE html>".len();

    fn dry_resolve(&mut self) {}

    async fn resolve(self) -> Self::AsyncOutput {
        self
    }

    fn to_html_with_buf(
        self,
        buf: &mut String,
        _position: &mut Position,
        _escape: bool,
        _mark_branches: bool,
        _extra_attrs: Vec<AnyAttribute>,
    ) {
        buf.push_str("<!DOCTYPE ");
        buf.push_str(self.value);
        buf.push('>');
    }

    fn hydrate<const FROM_SERVER: bool>(
        self,
        _cursor: &Cursor,
        _position: &PositionState,
    ) -> Self::State {
    }

    fn into_owned(self) -> Self::Owned {
        self
    }
}

/// An element that contains no interactivity, and whose contents can be known at compile time.
#[cfg(not(leptos_native))]
pub struct InertElement {
    html: Cow<'static, str>,
}

#[cfg(not(leptos_native))]
impl InertElement {
    /// Creates a new inert element.
    pub fn new(html: impl Into<Cow<'static, str>>) -> Self {
        Self { html: html.into() }
    }
}

/// Retained view state for [`InertElement`].
#[cfg(not(leptos_native))]
pub struct InertElementState(Cow<'static, str>, Element);

#[cfg(not(leptos_native))]
impl Mountable for InertElementState {
    fn unmount(&mut self) {
        self.1.unmount();
    }

    fn mount(&mut self, parent: &Element, marker: Option<&Node>) {
        self.1.mount(parent, marker)
    }

    fn insert_before_this(&self, child: &mut dyn Mountable) -> bool {
        self.1.insert_before_this(child)
    }

    fn elements(&self) -> Vec<crate::renderer::types::Element> {
        vec![self.1.clone()]
    }
}

#[cfg(not(leptos_native))]
impl Render for InertElement {
    type State = InertElementState;

    fn build(self) -> Self::State {
        let el = Rndr::create_element_from_html(self.html.clone());
        InertElementState(self.html, el)
    }

    fn rebuild(self, state: &mut Self::State) {
        let InertElementState(prev, el) = state;
        if &self.html != prev {
            let mut new_el = Rndr::create_element_from_html(self.html.clone());
            el.insert_before_this(&mut new_el);
            el.unmount();
            *el = new_el;
            *prev = self.html;
        }
    }
}

#[cfg(not(leptos_native))]
impl AddAnyAttr for InertElement {
    type Output<SomeNewAttr: Attribute> = Self;

    fn add_any_attr<NewAttr: Attribute>(
        self,
        _attr: NewAttr,
    ) -> Self::Output<NewAttr>
    where
        Self::Output<NewAttr>: RenderHtml,
    {
        panic!(
            "InertElement does not support adding attributes. It should only \
             be used as a child, and not returned at the top level."
        )
    }
}

#[cfg(not(leptos_native))]
impl RenderHtml for InertElement {
    type AsyncOutput = Self;
    type Owned = Self;

    const MIN_LENGTH: usize = 0;

    fn html_len(&self) -> usize {
        self.html.len()
    }

    fn dry_resolve(&mut self) {}

    async fn resolve(self) -> Self {
        self
    }

    fn to_html_with_buf(
        self,
        buf: &mut String,
        position: &mut Position,
        _escape: bool,
        _mark_branches: bool,
        _extra_attrs: Vec<AnyAttribute>,
    ) {
        buf.push_str(&self.html);
        *position = Position::NextChild;
    }

    fn hydrate<const FROM_SERVER: bool>(
        self,
        cursor: &Cursor,
        position: &PositionState,
    ) -> Self::State {
        let curr_position = position.get();
        if curr_position == Position::FirstChild {
            cursor.child();
        } else if curr_position != Position::Current {
            cursor.sibling();
        }
        let el = crate::renderer::types::Element::cast_from(cursor.current())
            .unwrap();
        position.set(Position::NextChild);
        InertElementState(self.html, el)
    }

    fn into_owned(self) -> Self::Owned {
        self
    }
}
