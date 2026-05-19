mod component_builder;
mod slot_helper;
mod utils;

use self::{
    component_builder::component_to_tokens,
    slot_helper::{get_slot, slot_to_tokens},
};
use convert_case::{
    Case::{Snake, UpperCamel},
    Casing,
};
use convert_case_extras::is_case;
use crate::parsing::{is_component_node, value_to_string};
use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use proc_macro_error2::abort;
use quote::{format_ident, quote, quote_spanned, ToTokens};
use rstml::node::{
    CustomNode, KVAttributeValue, KeyedAttribute, Node, NodeAttribute,
    NodeBlock, NodeElement, NodeName, NodeNameFragment,
};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};
use syn::{
    punctuated::Pair::{End, Punctuated},
    spanned::Spanned,
    Expr::{self, Tuple},
    ExprArray, ExprLit, ExprPath, ExprRange, Lit, LitStr, RangeLimits, Stmt,
};

pub fn render_view(
    nodes: &mut [Node],
    global_class: Option<&TokenTree>,
    view_marker: Option<String>,
) -> Option<TokenStream> {
    let (base, should_add_view) = match nodes.len() {
        0 => {
            let span = Span::call_site();
            (
                Some(quote_spanned! {
                    span => ()
                }),
                false,
            )
        }
        1 => (
            node_to_tokens(
                &mut nodes[0],
                None,
                global_class,
                view_marker.as_deref(),
                true,
            ),
            // only add View wrapper and view marker to a regular HTML
            // element or component, not to a <{..} /> attribute list
            match &nodes[0] {
                Node::Element(node) => !is_spread_marker(node),
                _ => false,
            },
        ),
        _ => (
            fragment_to_tokens(
                nodes,
                None,
                global_class,
                view_marker.as_deref(),
            ),
            true,
        ),
    };
    base.map(|view| {
        if !should_add_view {
            view
        } else if let Some(vm) = view_marker {
            quote! {
                ::leptos_native::prelude::View::new(
                    #view
                )
                .with_view_marker(#vm)
            }
        } else {
            quote! {
                ::leptos_native::prelude::View::new(
                    #view
                )
            }
        }
    })
}


fn element_children_to_tokens(
    nodes: &mut [Node<impl CustomNode>],
    parent_slots: Option<&mut HashMap<String, Vec<TokenStream>>>,
    global_class: Option<&TokenTree>,
    view_marker: Option<&str>,
) -> Option<TokenStream> {
    let children = children_to_tokens(
        nodes,
        parent_slots,
        global_class,
        view_marker,
        false,
    );
    if children.is_empty() {
        None
    } else if children.len() == 1 {
        let child = &children[0];
        Some(quote! {
            .child(
                #[allow(unused_braces)]
                { #child }
            )
        })
    } else if cfg!(feature = "__internal_erase_components") {
        Some(quote! {
            .child(
                ::leptos_native::tachys::view::iterators::StaticVec::from(vec![#(
                    ::leptos_native::prelude::IntoMaybeErased::into_maybe_erased(#children)
                ),*])
            )
        })
    } else if children.len() > 16 {
        // implementations of various traits used in routing and rendering are implemented for
        // tuples of sizes 0, 1, 2, 3, ... N. N varies but is > 16. The traits are also implemented
        // for tuples of tuples, so if we have more than 16 items, we can split them out into
        // multiple tuples.
        let chunks = children.chunks(16).map(|children| {
            quote! {
                (#(#children),*)
            }
        });
        Some(quote! {
            .child(
                (#(#chunks),*)
            )
        })
    } else {
        Some(quote! {
            .child(
                (#(#children),*)
            )
        })
    }
}

fn fragment_to_tokens(
    nodes: &mut [Node<impl CustomNode>],
    parent_slots: Option<&mut HashMap<String, Vec<TokenStream>>>,
    global_class: Option<&TokenTree>,
    view_marker: Option<&str>,
) -> Option<TokenStream> {
    let children = children_to_tokens(
        nodes,
        parent_slots,
        global_class,
        view_marker,
        true,
    );
    if children.is_empty() {
        None
    } else if children.len() == 1 {
        children.into_iter().next()
    } else if cfg!(feature = "__internal_erase_components") {
        Some(quote! {
            ::leptos_native::tachys::view::iterators::StaticVec::from(vec![#(
                ::leptos_native::prelude::IntoMaybeErased::into_maybe_erased(#children)
            ),*])
        })
    } else if children.len() > 16 {
        // implementations of various traits used in routing and rendering are implemented for
        // tuples of sizes 0, 1, 2, 3, ... N. N varies but is > 16. The traits are also implemented
        // for tuples of tuples, so if we have more than 16 items, we can split them out into
        // multiple tuples.
        let chunks = children.chunks(16).map(|children| {
            quote! {
                (#(#children),*)
            }
        });
        Some(quote! {
             (#(#chunks),*)
        })
    } else {
        Some(quote! {
            (#(#children),*)
        })
    }
}

fn children_to_tokens(
    nodes: &mut [Node<impl CustomNode>],
    parent_slots: Option<&mut HashMap<String, Vec<TokenStream>>>,
    global_class: Option<&TokenTree>,
    view_marker: Option<&str>,
    top_level: bool,
) -> Vec<TokenStream> {
    if nodes.len() == 1 {
        match node_to_tokens(
            &mut nodes[0],
            parent_slots,
            global_class,
            view_marker,
            top_level,
        ) {
            Some(tokens) => vec![tokens],
            None => vec![],
        }
    } else {
        let mut slots = HashMap::new();
        let nodes = nodes
            .iter_mut()
            .filter_map(|node| {
                node_to_tokens(
                    node,
                    Some(&mut slots),
                    global_class,
                    view_marker,
                    top_level,
                )
            })
            .collect();
        if let Some(parent_slots) = parent_slots {
            for (slot, mut values) in slots.drain() {
                parent_slots
                    .entry(slot)
                    .and_modify(|entry| entry.append(&mut values))
                    .or_insert(values);
            }
        }
        nodes
    }
}

fn node_to_tokens(
    node: &mut Node<impl CustomNode>,
    parent_slots: Option<&mut HashMap<String, Vec<TokenStream>>>,
    global_class: Option<&TokenTree>,
    view_marker: Option<&str>,
    _top_level: bool,
) -> Option<TokenStream> {
    match node {
        Node::Comment(_) => None,
        Node::Doctype(node) => {
            let value = node.value.to_string_best();
            Some(quote! { ::leptos_native::tachys::html::doctype(#value) })
        }
        Node::Fragment(fragment) => fragment_to_tokens(
            &mut fragment.children,
            parent_slots,
            global_class,
            view_marker,
        ),
        Node::Block(block) => {
            // Native: emit the bare block. Upstream wrapped this in
            // `IntoRender::into_render(...)` to normalize the value
            // into Render<R>, but that adds an R type parameter the
            // surrounding context often can't infer (e.g.
            // <label>{closure}</label> where label's .child() takes
            // IntoMaybeReactive<String>, not Render). The blanket
            // `impl<R, T: Render<R>> IntoRender<R> for T` is identity
            // anyway, so dropping the wrap is type-equivalent
            // for paths that ARE generic in R.
            Some(quote! { (#block) })
        }
        Node::Text(text) => Some(text_to_tokens(&text.value)),
        Node::RawText(raw) => {
            let text = raw.to_string_best();
            let text = syn::LitStr::new(&text, raw.span());
            Some(text_to_tokens(&text))
        }
        Node::Element(el_node) => element_to_tokens(
            el_node,
            parent_slots,
            global_class,
            view_marker,
        ),
        Node::Custom(node) => Some(node.to_token_stream()),
    }
}

fn text_to_tokens(text: &LitStr) -> TokenStream {
    // on nightly, can use static string optimization
    if cfg!(all(feature = "nightly", rustc_nightly)) {
        quote! {
            ::leptos_native::tachys::view::static_types::Static::<#text>
        }
    }
    // otherwise, just use the literal string
    else {
        quote! { #text }
    }
}

pub(crate) fn element_to_tokens(
    node: &mut NodeElement<impl CustomNode>,
    parent_slots: Option<&mut HashMap<String, Vec<TokenStream>>>,
    global_class: Option<&TokenTree>,
    view_marker: Option<&str>,
) -> Option<TokenStream> {
    // attribute sorting:
    //
    // the `class` and `style` attributes overwrite individual `class:` and `style:` attributes
    // when they are set. as a result, we're going to sort the attributes so that `class` and
    // `style` always come before all other attributes.

    // if there's a spread marker, we don't want to move `class` or `style` before it
    // so let's only sort attributes that come *before* a spread marker
    let spread_position = node
        .attributes()
        .iter()
        .position(|n| match n {
            NodeAttribute::Block(node) => as_spread_attr(node).is_some(),
            _ => false,
        })
        .unwrap_or_else(|| node.attributes().len());

    // now, sort the attributes
    node.attributes_mut()[0..spread_position].sort_by(|a, b| {
        let key_a = match a {
            NodeAttribute::Attribute(attr) => match &attr.key {
                NodeName::Path(attr) => {
                    attr.path.segments.first().map(|n| n.ident.to_string())
                }
                _ => None,
            },
            _ => None,
        };
        let key_b = match b {
            NodeAttribute::Attribute(attr) => match &attr.key {
                NodeName::Path(attr) => {
                    attr.path.segments.first().map(|n| n.ident.to_string())
                }
                _ => None,
            },
            _ => None,
        };

        if let NodeAttribute::Attribute(a) = a {
            if let Some(Tuple(_)) = a.value() {
                return Ordering::Greater;
            }
        }
        if let NodeAttribute::Attribute(b) = b {
            if let Some(Tuple(_)) = b.value() {
                return Ordering::Less;
            }
        }

        match (key_a.as_deref(), key_b.as_deref()) {
            (Some("class"), Some("class")) | (Some("style"), Some("style")) => {
                Ordering::Equal
            }
            (Some("class"), _) | (Some("style"), _) => Ordering::Less,
            (_, Some("class")) | (_, Some("style")) => Ordering::Greater,
            _ => Ordering::Equal,
        }
    });

    // check for duplicate attribute names and emit an error for all subsequent ones
    let mut names = HashSet::new();

    // allow multiple class=(...) or style=(...) attributes
    fn allow_multiples(name: &str, attr: &KeyedAttribute) -> bool {
        (name == "class" || name == "style")
            && matches!(attr.value(), Some(Expr::Tuple(..)))
    }

    for attr in node.attributes() {
        if let NodeAttribute::Attribute(attr) = attr {
            let mut name = attr.key.to_string();
            match tuple_name(&name, attr) {
                TupleName::None => {}
                TupleName::Str(tuple_name) => {
                    name.push(':');
                    name.push_str(&tuple_name);
                }
                TupleName::Array(names) => {
                    for tuple_name in names {
                        name.push(':');
                        name.push_str(&tuple_name);
                    }
                }
            }
            if names.contains(&name) && !allow_multiples(&name, attr) {
                proc_macro_error2::emit_error!(
                    attr.span(),
                    format!("This element already has a `{name}` attribute.")
                );
            } else {
                names.insert(name);
            }
        }
    }

    let name = node.name();
    if is_component_node(node) {
        if let Some(slot) = get_slot(node) {
            let slot = slot.clone();
            slot_to_tokens(
                node,
                &slot,
                parent_slots,
                global_class,
            );
            None
        } else {
            Some(component_to_tokens(node, global_class))
        }
    } else if is_spread_marker(node) {
        let mut attributes = Vec::new();
        let mut additions = Vec::new();
        for node in node.attributes() {
            match node {
                NodeAttribute::Block(block) => {
                    if let NodeBlock::ValidBlock(block) = block {
                        match block.stmts.first() {
                            Some(Stmt::Expr(
                                Expr::Range(ExprRange {
                                    start: None,
                                    limits: RangeLimits::HalfOpen(_),
                                    end: Some(end),
                                    ..
                                }),
                                _,
                            )) => {
                                additions.push(quote! { #end });
                            }
                            _ => {
                                additions.push(quote! { #block });
                            }
                        }
                    } else {
                        additions.push(quote! { #block });
                    }
                }
                NodeAttribute::Attribute(node) => {
                    if let Some(content) = attribute_absolute(node, true) {
                        attributes.push(content);
                    }
                }
            }
        }

        if cfg!(feature = "__internal_erase_components") {
            Some(quote! {
                vec![#(#attributes.into_any_attr(),)*]
                #(.add_any_attr(#additions))*
            })
        } else {
            Some(quote! {
                (#(#attributes,)*)
                #(.add_any_attr(#additions))*
            })
        }
    } else {
        let tag = name.to_string();
        let is_custom = is_custom_element(&tag);
        // Native-only: every tag routes through `tachys::html::element::*`.
        // Upstream Leptos special-cased SVG and MathML element names
        // to route through `tachys::svg::*` / `tachys::mathml::*` and
        // to emit `.attr(name, value)` for every attribute. On native
        // there are no SVG or MathML renderers and no untyped `.attr()`
        // shim, so we drop the special-case routing entirely and use
        // raw identifiers for the two tags whose names collide with
        // Rust keywords (`use`, `switch`).
        let name = if is_custom {
            let name = node.name().to_string();
            let custom = Ident::new("custom", name.span());
            quote_spanned! { node.name().span() => ::leptos_native::tachys::html::element::#custom(#name) }
        } else {
            let ident = match tag.as_str() {
                "use" | "use_" => Ident::new_raw("use", name.span()).to_token_stream(),
                "switch" => Ident::new_raw("switch", name.span()).to_token_stream(),
                _ => name.to_token_stream(),
            };
            quote_spanned! { node.name().span() => ::leptos_native::tachys::html::element::#ident() }
        };

        let attributes = node.attributes();
        let attributes = if attributes.len() == 1 {
            Some(attribute_to_tokens(
                &attributes[0],
                global_class,
                is_custom,
            ))
        } else {
            let nodes = attributes.iter().map(|node| {
                attribute_to_tokens(node, global_class, is_custom)
            });
            Some(quote! {
                #(#nodes)*
            })
        };

        let global_class_expr = global_class.map(|class| {
            quote! { .class((#class, true)) }
        });

        let self_closing = is_self_closing(node);
        let children = if !self_closing {
            element_children_to_tokens(
                &mut node.children,
                parent_slots,
                global_class,
                view_marker,
            )
        } else {
            if !node.children.is_empty() {
                let name = node.name();
                proc_macro_error2::emit_error!(
                    name.span(),
                    format!(
                        "Self-closing elements like <{name}> cannot have \
                         children."
                    )
                );
            };
            None
        };

        // attributes are placed second because this allows `inner_html`
        // to object if there are already children
        Some(quote! {
            #name
            #children
            #attributes
            #global_class_expr
        })
    }
}

fn is_spread_marker(node: &NodeElement<impl CustomNode>) -> bool {
    match node.name() {
        NodeName::Block(block) => matches!(
            block.stmts.first(),
            Some(Stmt::Expr(
                Expr::Range(ExprRange {
                    start: None,
                    limits: RangeLimits::HalfOpen(_),
                    end: None,
                    ..
                }),
                _,
            ))
        ),
        _ => false,
    }
}

fn as_spread_attr(node: &NodeBlock) -> Option<Option<&Expr>> {
    if let NodeBlock::ValidBlock(block) = node {
        match block.stmts.first() {
            Some(Stmt::Expr(
                Expr::Range(ExprRange {
                    start: None,
                    limits: RangeLimits::HalfOpen(_),
                    end,
                    ..
                }),
                _,
            )) => Some(end.as_deref()),
            _ => None,
        }
    } else {
        None
    }
}

fn attribute_to_tokens(
    node: &NodeAttribute,
    global_class: Option<&TokenTree>,
    is_custom: bool,
) -> TokenStream {
    match node {
        NodeAttribute::Block(node) => as_spread_attr(node)
            .flatten()
            .map(|end| {
                quote! {
                    .add_any_attr(#end)
                }
            })
            .unwrap_or_else(|| {
                quote! {
                    .add_any_attr(#[allow(unused_braces)] { #node })
                }
            }),
        NodeAttribute::Attribute(node) => {
            let name = node.key.to_string();
            if name == "node_ref" {
                let node_ref = match &node.key {
                    NodeName::Path(path) => path.path.get_ident(),
                    _ => unreachable!(),
                };
                let value = attribute_value(node, false);
                quote! {
                    .#node_ref(#value)
                }
            } else if let Some(name) = name.strip_prefix("use:") {
                directive_call_from_attribute_node(node, name)
            } else if let Some(name) = name.strip_prefix("on:") {
                event_to_tokens(name, node)
            } else if let Some(name) = name.strip_prefix("bind:") {
                two_way_binding_to_tokens(name, node)
            } else if let Some(name) = name.strip_prefix("class:") {
                let class = match &node.key {
                    NodeName::Punctuated(parts) => &parts[0],
                    _ => unreachable!(),
                };
                class_to_tokens(node, class.into_token_stream(), Some(name))
            } else if name == "class" {
                let class = match &node.key {
                    NodeName::Path(path) => path.path.get_ident(),
                    _ => unreachable!(),
                };
                class_to_tokens(node, class.into_token_stream(), None)
            } else if let Some(name) = name.strip_prefix("style:") {
                let style = match &node.key {
                    NodeName::Punctuated(parts) => &parts[0],
                    _ => unreachable!(),
                };
                style_to_tokens(node, style.into_token_stream(), Some(name))
            } else if name == "style" {
                let style = match &node.key {
                    NodeName::Path(path) => path.path.get_ident(),
                    _ => unreachable!(),
                };
                style_to_tokens(node, style.into_token_stream(), None)
            } else if let Some(name) = name.strip_prefix("prop:") {
                let prop = match &node.key {
                    NodeName::Punctuated(parts) => &parts[0],
                    _ => unreachable!(),
                };
                prop_to_tokens(node, prop.into_token_stream(), name)
            }
            // Unchecked attributes go through `.attr(name, value)`:
            // 1) custom elements, which can have any attributes
            // 2) custom + data attributes (anything hyphenated, except `aria-*`)
            else if is_custom ||
                (name.contains('-') && !name.starts_with("aria-"))
            {
                let value = attribute_value(node, true);
                quote! {
                    .attr(#name, #value)
                }
            } else {
                let key = attribute_name(&node.key);
                let value = attribute_value(node, true);

                // special case of global_class and class attribute
                if &node.key.to_string() == "class"
                    && global_class.is_some()
                    && node.value().and_then(value_to_string).is_none()
                {
                    let span = node.key.span();
                    proc_macro_error2::emit_error!(span, "Combining a global class (view! { class = ... }) \
            and a dynamic `class=` attribute on an element causes runtime inconsistencies. You can \
            toggle individual classes dynamically with the `class:name=value` syntax. \n\nSee this issue \
            for more information and an example: https://github.com/leptos-rs/leptos/issues/773")
                };

                quote! {
                    .#key(#value)
                }
            }
        }
    }
}

/// Returns attribute values with an absolute path
pub(crate) fn attribute_absolute(
    node: &KeyedAttribute,
    after_spread: bool,
) -> Option<TokenStream> {
    let key = node.key.to_string();
    let contains_dash = key.contains('-');
    let attr_colon = key.starts_with("attr:")
        || key.starts_with("style:")
        || key.starts_with("class:")
        || key.starts_with("prop:")
        || key.starts_with("use:");
    // anything that follows the x:y pattern
    match &node.key {
        NodeName::Punctuated(parts) if !contains_dash || attr_colon => {
            if parts.len() >= 2 {
                let id = &parts[0];
                match id {
                    NodeNameFragment::Ident(id) => {
                        // ignore `let:` and `clone:`
                        if id == "let" || id == "clone" {
                            None
                        } else if id == "attr" {
                            let value = attribute_value(node, true);
                            let multipart = parts.len() > 2;
                            let key = &parts[1];
                            let key_name = key.to_string();
                            if key_name == "class" || key_name == "style" {
                                Some(
                                    quote! { ::leptos_native::tachys::html::#key::#key(#value) },
                                )
                            } else if key_name == "aria" {
                                let value = attribute_value(node, true);
                                let mut parts_iter = parts.iter();
                                parts_iter.next();
                                let fn_name = parts_iter.map(|p| p.to_string()).collect::<Vec<String>>().join("_");
                                let key = Ident::new(&fn_name, key.span());
                                Some(
                                    quote! { ::leptos_native::tachys::html::attribute::#key(#value) },
                                )
                            } else if multipart {
                                // e.g., attr:data-foo="bar"
                                let key_name = parts.pairs().skip(1).map(|p| match p {
                                    Punctuated(n, p) => format!("{n}{p}"),
                                    End(n) => n.to_string(),
                                }).collect::<String>();
                                Some(
                                    quote! { ::leptos_native::tachys::html::attribute::custom::custom_attribute(#key_name, #value) },
                                )
                            } else {
                                Some(
                                    quote! { ::leptos_native::tachys::html::attribute::#key(#value) },
                                )
                            }
                        } else if id == "use" {
                            let key = &parts[1];
                            let param = if let Some(value) = node.value() {
                                quote!(#value)
                            } else {
                                quote_spanned!(node.key.span()=> ().into())
                            };
                            Some(
                                quote! {
                                    ::leptos_native::tachys::html::directive::directive(
                                        #key,
                                        #[allow(clippy::useless_conversion)] #param
                                    )
                                },
                            )
                        } else if id == "style" || id == "class" {
                            let value = attribute_value(node, false);
                            let key = &node.key.to_string();
                            let key = key
                                .replacen("style:", "", 1)
                                .replacen("class:", "", 1);
                            Some(
                                quote! { ::leptos_native::tachys::html::#id::#id((#key, #value)) },
                            )
                        } else if id == "prop" {
                            let value = attribute_value(node, false);
                            let key = &node.key.to_string();
                            let key = key.replacen("prop:", "", 1);
                            Some(
                                quote! { ::leptos_native::tachys::html::property::#id(#key, #value) },
                            )
                        } else if id == "on" {
                            let key = &node.key.to_string();
                            let key = key.replacen("on:", "", 1);
                            let (on, ty, handler) =
                                event_type_and_handler(&key, node);
                            Some(
                                quote! { ::leptos_native::tachys::html::event::#on(#ty, #handler) },
                            )
                        } else {
                            proc_macro_error2::abort!(
                                id.span(),
                                &format!(
                                    "`{id}:` syntax is not supported on \
                                     components"
                                )
                            );
                        }
                    }
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => after_spread.then(|| {
            let key = attribute_name(&node.key);
            let value = &node.value();
            let name = &node.key.to_string();
            if name == "class" || name == "style" {
                quote! {
                    ::leptos_native::tachys::html::#key::#key(#value)
                }
            }
            else if name.contains('-') && !name.starts_with("aria-") {
                quote! {
                    ::leptos_native::tachys::html::attribute::custom::custom_attribute(#name, #value)
                }
            }
            else if name == "node_ref" {
                quote! {
                    ::leptos_native::tachys::html::node_ref::#key(#value)
                }
            }
            else {
                quote! {
                    ::leptos_native::tachys::html::attribute::#key(#value)
                }
            }
        }),
    }
}

pub(crate) fn two_way_binding_to_tokens(
    name: &str,
    node: &KeyedAttribute,
) -> TokenStream {
    let value = attribute_value(node, false);

    let ident =
        format_ident!("{}", name.to_case(UpperCamel), span = node.key.span());

    if name == "group" {
        quote! {
            .bind(leptos_native::tachys::reactive_graph::bind::#ident, #value)
        }
    } else {
        quote! {
            .bind(::leptos_native::attr::#ident, #value)
        }
    }
}

pub(crate) fn event_to_tokens(
    name: &str,
    node: &KeyedAttribute,
) -> TokenStream {
    let (on, event_type, handler) = event_type_and_handler(name, node);

    quote! {
        .#on(#event_type, #handler)
    }
}

pub(crate) fn event_type_and_handler(
    name: &str,
    node: &KeyedAttribute,
) -> (TokenStream, TokenStream, TokenStream) {
    let handler = attribute_value(node, false);

    let (event_type, is_custom, options) = parse_event_name(name);

    let event_name_ident = match &node.key {
        NodeName::Punctuated(parts) => {
            if parts.len() >= 2 {
                Some(&parts[1])
            } else {
                None
            }
        }
        _ => unreachable!(),
    };
    let undelegated_ident = match &node.key {
        NodeName::Punctuated(parts) => {
            parts.iter().find(|part| part.to_string() == "undelegated")
        }
        _ => unreachable!(),
    };
    let capture_ident = match &node.key {
        NodeName::Punctuated(parts) => {
            parts.iter().find(|part| part.to_string() == "capture")
        }
        _ => unreachable!(),
    };
    let on = match &node.key {
        NodeName::Punctuated(parts) => &parts[0],
        _ => unreachable!(),
    };
    let on = if options.targeted {
        Ident::new("on_target", on.span()).to_token_stream()
    } else {
        on.to_token_stream()
    };
    let event_type = if is_custom {
        event_type
    } else if let Some(ev_name) = event_name_ident {
        quote! { #ev_name }
    } else {
        event_type
    };

    let event_type = quote! {
        ::leptos_native::tachys::html::event::#event_type
    };
    let event_type = if options.captured {
        let capture = if let Some(capture) = capture_ident {
            quote! { #capture }
        } else {
            quote! { capture }
        };
        quote! { ::leptos_native::tachys::html::event::#capture(#event_type) }
    } else {
        event_type
    };

    let event_type = if options.undelegated {
        let undelegated = if let Some(undelegated) = undelegated_ident {
            quote! { #undelegated }
        } else {
            quote! { undelegated }
        };
        quote! { ::leptos_native::tachys::html::event::#undelegated(#event_type) }
    } else {
        event_type
    };

    (on, event_type, handler)
}

fn class_to_tokens(
    node: &KeyedAttribute,
    class: TokenStream,
    class_name: Option<&str>,
) -> TokenStream {
    // case of class=(["foo", "bar"], /* something */)
    // just expands to multiple uses of class:
    if let Some(Tuple(tuple)) = node.value() {
        if tuple.elems.len() == 2 {
            let name = &tuple.elems[0];
            let value = &tuple.elems[1];
            if let Expr::Array(ExprArray { elems, .. }) = name {
                return elems
                    .iter()
                    .map(|elem| match elem {
                        Expr::Lit(ExprLit {
                            lit: Lit::Str(s), ..
                        }) => quote! {
                            .#class((#s, #value))
                        },
                        _ => proc_macro_error2::abort!(
                            elem.span(),
                            "invalid name"
                        ),
                    })
                    .collect();
            }
        }
    }

    // default case
    let value = attribute_value(node, false);
    if let Some(class_name) = class_name {
        quote! {
            .#class((#class_name, #value))
        }
    } else {
        quote! {
            .#class(#value)
        }
    }
}

fn style_to_tokens(
    node: &KeyedAttribute,
    style: TokenStream,
    style_name: Option<&str>,
) -> TokenStream {
    let value = attribute_value(node, false);
    if let Some(style_name) = style_name {
        quote! {
            .#style((#style_name, #value))
        }
    } else {
        quote! {
            .#style(#value)
        }
    }
}

fn prop_to_tokens(
    node: &KeyedAttribute,
    prop: TokenStream,
    key: &str,
) -> TokenStream {
    let value = attribute_value(node, false);
    quote! {
        .#prop(#key, #value)
    }
}

fn is_custom_element(tag: &str) -> bool {
    tag.contains('-')
}

fn is_self_closing(node: &NodeElement<impl CustomNode>) -> bool {
    // self-closing tags
    // https://developer.mozilla.org/en-US/docs/Glossary/Empty_element
    [
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link",
        "meta", "param", "source", "track", "wbr",
    ]
    .binary_search(&node.name().to_string().as_str())
    .is_ok()
}

fn parse_event(event_name: &str) -> (String, EventNameOptions) {
    let undelegated = event_name.contains(":undelegated");
    let targeted = event_name.contains(":target");
    let captured = event_name.contains(":capture");
    let event_name = event_name
        .replace(":undelegated", "")
        .replace(":target", "")
        .replace(":capture", "");
    (
        event_name,
        EventNameOptions {
            undelegated,
            targeted,
            captured,
        },
    )
}

/// Escapes Rust keywords that are also HTML attribute names
/// to their raw-identifier form.
fn attribute_name(name: &NodeName) -> TokenStream {
    let s = name.to_string();
    if s == "as" || s == "async" || s == "loop" || s == "for" || s == "type" {
        Ident::new_raw(&s, name.span()).to_token_stream()
    } else if s.starts_with("aria-") {
        Ident::new(&s.replace('-', "_"), name.span()).to_token_stream()
    } else {
        name.to_token_stream()
    }
}

fn attribute_value(
    attr: &KeyedAttribute,
    is_attribute_proper: bool,
) -> TokenStream {
    match attr.possible_value.to_value() {
        None => quote! { true },
        Some(value) => match &value.value {
            KVAttributeValue::Expr(expr) => {
                if let Expr::Lit(lit) = expr {
                    if cfg!(all(feature = "nightly", rustc_nightly)) {
                        if let Lit::Str(str) = &lit.lit {
                            return quote! {
                                ::leptos_native::tachys::view::static_types::Static::<#str>
                            };
                        }
                    }
                }

                if matches!(expr, Expr::Lit(_)) || !is_attribute_proper {
                    quote! {
                        #expr
                    }
                } else {
                    quote! {
                        ::leptos_native::prelude::IntoAttributeValue::into_attribute_value(#expr)
                    }
                }
            }
            // any value in braces: expand as-is to give proper r-a support
            KVAttributeValue::InvalidBraced(block) => {
                if is_attribute_proper {
                    quote! {
                        ::leptos_native::prelude::IntoAttributeValue::into_attribute_value(#block)
                    }
                } else {
                    quote! {
                        #block
                    }
                }
            }
        },
    }
}

// Keep list alphabetized for binary search
const TYPED_EVENTS: [&str; 129] = [
    "DOMContentLoaded",
    "abort",
    // Native-only: fired by `<menu_item>` when activated via mouse,
    // keyboard shortcut, voice control, or accessibility. Mapped on
    // cocoa to NSMenuItem.action / on GTK to gio::Action.activate.
    "action",
    "afterprint",
    "animationcancel",
    "animationend",
    "animationiteration",
    "animationstart",
    "auxclick",
    "beforeinput",
    "beforeprint",
    "beforeunload",
    "blur",
    "canplay",
    "canplaythrough",
    "change",
    "click",
    "close",
    "commit",
    "compositionend",
    "compositionstart",
    "compositionupdate",
    "contextmenu",
    "copy",
    "cuechange",
    "cut",
    "dblclick",
    "devicemotion",
    "deviceorientation",
    "drag",
    "dragend",
    "dragenter",
    "dragleave",
    "dragover",
    "dragstart",
    "drop",
    "durationchange",
    "emptied",
    "ended",
    "error",
    "focus",
    "focusin",
    "focusout",
    "formdata",
    "fullscreenchange",
    "fullscreenerror",
    "gamepadconnected",
    "gamepaddisconnected",
    "gotpointercapture",
    "hashchange",
    "input",
    "invalid",
    "keydown",
    "keypress",
    "keyup",
    "languagechange",
    "load",
    "loadeddata",
    "loadedmetadata",
    "loadstart",
    "lostpointercapture",
    "message",
    "messageerror",
    "mousedown",
    "mouseenter",
    "mouseleave",
    "mousemove",
    "mouseout",
    "mouseover",
    "mouseup",
    "offline",
    "online",
    "orientationchange",
    "pagehide",
    "pageshow",
    "paste",
    "pause",
    "play",
    "playing",
    "pointercancel",
    "pointerdown",
    "pointerenter",
    "pointerleave",
    "pointerlockchange",
    "pointerlockerror",
    "pointermove",
    "pointerout",
    "pointerover",
    "pointerup",
    "popstate",
    "progress",
    "ratechange",
    "readystatechange",
    "rejectionhandled",
    "reset",
    "resize",
    "scroll",
    "scrollend",
    "securitypolicyviolation",
    "seeked",
    "seeking",
    "select",
    "selectionchange",
    "selectstart",
    "slotchange",
    "stalled",
    "storage",
    "submit",
    "suspend",
    "timeupdate",
    "toggle",
    "touchcancel",
    "touchend",
    "touchmove",
    "touchstart",
    "transitioncancel",
    "transitionend",
    "transitionrun",
    "transitionstart",
    "unhandledrejection",
    "unload",
    "visibilitychange",
    "volumechange",
    "waiting",
    "webkitanimationend",
    "webkitanimationiteration",
    "webkitanimationstart",
    "webkittransitionend",
    "wheel",
];

const CUSTOM_EVENT: &str = "Custom";

#[derive(Debug)]
pub(crate) struct EventNameOptions {
    undelegated: bool,
    targeted: bool,
    captured: bool,
}

pub(crate) fn parse_event_name(
    name: &str,
) -> (TokenStream, bool, EventNameOptions) {
    let (name, options) = parse_event(name);

    let (event_type, is_custom) = TYPED_EVENTS
        .binary_search(&name.as_str())
        .map(|_| (name.as_str(), false))
        .unwrap_or((CUSTOM_EVENT, true));

    let Ok(event_type) = event_type.parse::<TokenStream>() else {
        abort!(event_type, "couldn't parse event name");
    };

    let event_type = if is_custom {
        quote! { Custom::new(#name) }
    } else {
        event_type
    };
    (event_type, is_custom, options)
}

fn convert_to_snake_case(name: String) -> String {
    if !is_case(&name, Snake) {
        name.to_case(Snake)
    } else {
        name
    }
}

pub(crate) fn ident_from_tag_name(tag_name: &NodeName) -> Ident {
    match tag_name {
        NodeName::Path(path) => path
            .path
            .segments
            .iter()
            .next_back()
            .map(|segment| segment.ident.clone())
            .expect("element needs to have a name"),
        NodeName::Block(_) => {
            let span = tag_name.span();
            proc_macro_error2::emit_error!(
                span,
                "blocks not allowed in tag-name position"
            );
            Ident::new("", span)
        }
        _ => Ident::new(
            &tag_name.to_string().replace(['-', ':'], "_"),
            tag_name.span(),
        ),
    }
}

pub(crate) fn full_path_from_tag_name(tag_name: &NodeName) -> Option<ExprPath> {
    match tag_name {
        NodeName::Path(path) => Some(path.clone()),
        NodeName::Block(_) => {
            let span = tag_name.span();
            proc_macro_error2::emit_error!(
                span,
                "blocks not allowed in tag-name position"
            );
            None
        }
        _ => {
            let span = tag_name.span();
            proc_macro_error2::emit_error!(
                span,
                "punctuated names not allowed in slots"
            );
            None
        }
    }
}

pub(crate) fn directive_call_from_attribute_node(
    attr: &KeyedAttribute,
    directive_name: &str,
) -> TokenStream {
    let handler = syn::Ident::new(directive_name, attr.key.span());

    let param = if let Some(value) = attr.value() {
        quote!(#value)
    } else {
        quote_spanned!(attr.key.span()=> ().into())
    };

    quote! { .directive(#handler, #[allow(clippy::useless_conversion)] #param) }
}

fn tuple_name(name: &str, node: &KeyedAttribute) -> TupleName {
    if name == "style" || name == "class" {
        if let Some(Tuple(tuple)) = node.value() {
            {
                if tuple.elems.len() == 2 {
                    let style_name = &tuple.elems[0];
                    if let Expr::Lit(ExprLit {
                        lit: Lit::Str(s), ..
                    }) = style_name
                    {
                        return TupleName::Str(s.value());
                    } else if let Expr::Array(ExprArray { elems, .. }) =
                        style_name
                    {
                        return TupleName::Array(
                            elems
                                .iter()
                                .filter_map(|elem| match elem {
                                    Expr::Lit(ExprLit {
                                        lit: Lit::Str(s),
                                        ..
                                    }) => Some(s.value()),
                                    _ => proc_macro_error2::abort!(
                                        elem.span(),
                                        "invalid name"
                                    ),
                                })
                                .collect(),
                        );
                    }
                }
            }
        }
    }

    TupleName::None
}

#[derive(Debug, PartialEq, Eq)]
enum TupleName {
    None,
    Str(String),
    Array(Vec<String>),
}
