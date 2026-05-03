#[cfg(not(target_os = "macos"))]
use self::attribute::Attribute;
#[cfg(not(target_os = "macos"))]
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
#[cfg(not(target_os = "macos"))]
use attribute::any_attribute::AnyAttribute;
#[cfg(not(target_os = "macos"))]
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
#[cfg(not(target_os = "macos"))]
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
#[cfg(not(target_os = "macos"))]
pub mod class;
/// Types for creating user-defined attributes with custom behavior (directives).
#[cfg(not(target_os = "macos"))]
pub mod directive;
/// Types for HTML elements (web only — Cocoa elements live in their own
/// module, defined in Stage 5).
#[cfg(not(target_os = "macos"))]
pub mod element;
/// On macOS, `tachys::html::element` is a thin facade re-exporting
/// the Cocoa builders, so that `view!{}` macro emissions like
/// `::leptos::tachys::html::element::button()` resolve correctly.
#[cfg(target_os = "macos")]
pub mod element_macos;
#[cfg(target_os = "macos")]
pub use element_macos as element;

/// Types for DOM events.
#[cfg(not(target_os = "macos"))]
pub mod event;
/// On macOS, `tachys::html::event` is a thin facade providing event
/// descriptors and the `on(event, handler)` wrapper that maps to our
/// Cocoa target/action infrastructure.
#[cfg(target_os = "macos")]
pub mod event_macos;
#[cfg(target_os = "macos")]
pub use event_macos as event;
/// Types for adding interactive islands to inert HTML pages.
#[cfg(not(target_os = "macos"))]
pub mod islands;
/// Types for accessing a reference to an HTML element.
#[cfg(not(target_os = "macos"))]
pub mod node_ref;
/// Types for DOM properties.
#[cfg(not(target_os = "macos"))]
pub mod property;
/// Types for the `style` attribute and individual style manipulation.
#[cfg(not(target_os = "macos"))]
pub mod style;

/// A `<!DOCTYPE>` declaration. Web-only — disabled on native targets
/// since the renderer has no concept of inert HTML or doctypes.
#[cfg(not(target_os = "macos"))]
pub struct Doctype {
    value: &'static str,
}

/// Creates a `<!DOCTYPE>`.
#[cfg(not(target_os = "macos"))]
pub fn doctype(value: &'static str) -> Doctype {
    Doctype { value }
}

#[cfg(not(target_os = "macos"))]
impl Render for Doctype {
    type State = ();

    fn build(self) -> Self::State {}

    fn rebuild(self, _state: &mut Self::State) {}
}

#[cfg(not(target_os = "macos"))]
no_attrs!(Doctype);

#[cfg(not(target_os = "macos"))]
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
#[cfg(not(target_os = "macos"))]
pub struct InertElement {
    html: Cow<'static, str>,
}

#[cfg(not(target_os = "macos"))]
impl InertElement {
    /// Creates a new inert element.
    pub fn new(html: impl Into<Cow<'static, str>>) -> Self {
        Self { html: html.into() }
    }
}

/// Retained view state for [`InertElement`].
#[cfg(not(target_os = "macos"))]
pub struct InertElementState(Cow<'static, str>, Element);

#[cfg(not(target_os = "macos"))]
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

#[cfg(not(target_os = "macos"))]
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

#[cfg(not(target_os = "macos"))]
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

#[cfg(not(target_os = "macos"))]
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
