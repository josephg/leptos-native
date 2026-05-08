//! Small helpers used by `view!`/`component!` macro expansion. Vendored from
//! upstream `leptos_hot_reload::parsing` (MIT) — that crate was deleted from
//! this fork in Phase 4 since hot-reloading is web-only.

use rstml::node::{CustomNode, NodeElement, NodeName};
use std::path::Path;

/// True if a node's tag name starts with an uppercase ASCII letter — the
/// convention `view!` uses to distinguish components (`<MyThing/>`) from
/// HTML/native elements (`<button/>`).
#[must_use]
pub fn is_component_tag_name(name: &NodeName) -> bool {
    match name {
        NodeName::Path(path) => {
            !path.path.segments.is_empty()
                && path
                    .path
                    .segments
                    .last()
                    .unwrap()
                    .ident
                    .to_string()
                    .starts_with(|c: char| c.is_ascii_uppercase())
        }
        NodeName::Block(_) | NodeName::Punctuated(_) => false,
    }
}

#[must_use]
pub fn is_component_node(node: &NodeElement<impl CustomNode>) -> bool {
    is_component_tag_name(node.name())
}

/// Reduces a literal expression to its source string form. Returns `None`
/// for non-literals or literal kinds we don't handle (e.g. byte strings).
#[must_use]
pub fn value_to_string(value: &syn::Expr) -> Option<String> {
    match value {
        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Str(s) => Some(s.value()),
            syn::Lit::Char(c) => Some(c.value().to_string()),
            syn::Lit::Int(i) => Some(i.base10_digits().to_string()),
            syn::Lit::Float(f) => Some(f.base10_digits().to_string()),
            _ => None,
        },
        _ => None,
    }
}

/// Stable ID for a `view!{}` invocation, used for hot-reload patching.
/// Native UIs don't actually do hot-reload (the leptos_hot_reload crate is
/// gone), but the macro emits this in tracking calls and we keep a stub.
pub fn span_to_stable_id(path: impl AsRef<Path>, line: usize) -> String {
    let file = path
        .as_ref()
        .to_str()
        .unwrap_or_default()
        .replace(['/', '\\'], "-");
    format!("{file}-{line}")
}
