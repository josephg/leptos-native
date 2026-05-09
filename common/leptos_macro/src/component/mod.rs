//! `#[component]` macro implementation.
//!
//! Split across submodules for navigability:
//! - [`docs`] — `Docs` + `UnknownAttrs` (capture and re-emit doc
//!   comments + unknown attributes attached to the component fn).
//! - [`prop`] — `Prop`, `PropOpt`, `TypedBuilderOpts`, and the
//!   helpers that turn props into TypedBuilder fields and component
//!   fn doc lines.
//! - [`util`] — type-level helpers (`is_option`, `unwrap_option`,
//!   `convert_from_snake_case`, `drain_filter`,
//!   `convert_impl_trait_to_generic`, `unmodified_fn_name_from_fn_name`,
//!   `maybe_modify_return_type`).
//!
//! This file holds the two parse models — [`Model`] (the real one)
//! and [`DummyModel`] (the lenient one used for IDE auto-complete on
//! syntax errors) — and the `Model::to_tokens` codegen that drives
//! the macro output.

mod docs;
mod prop;
mod util;

pub use docs::Docs;
pub use util::{
    convert_from_snake_case, drain_filter, is_option,
    unmodified_fn_name_from_fn_name, unwrap_option,
};

use docs::UnknownAttrs;
use prop::{
    generate_component_fn_prop_docs, prop_builder_fields, prop_names, Prop,
};
use util::{convert_impl_trait_to_generic, maybe_modify_return_type};

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote, ToTokens, TokenStreamExt};
use syn::{
    parse::Parse, parse_quote, spanned::Spanned, Attribute, FnArg, Item,
    ItemFn, LitStr, Meta, ReturnType, Signature, Stmt, Visibility,
};

pub struct Model {
    is_transparent: bool,
    docs: Docs,
    unknown_attrs: UnknownAttrs,
    vis: Visibility,
    name: Ident,
    props: Vec<Prop>,
    body: ItemFn,
    ret: ReturnType,
}

impl Parse for Model {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut item = ItemFn::parse(input)?;
        maybe_modify_return_type(&mut item.sig.output);

        convert_impl_trait_to_generic(&mut item.sig);

        let docs = Docs::new(&item.attrs);
        let unknown_attrs = UnknownAttrs::new(&item.attrs);

        let props = item
            .sig
            .inputs
            .clone()
            .into_iter()
            .map(Prop::new)
            .collect::<Vec<_>>();

        // Remove the `#[doc = ""]` and `#[builder(_)]` attrs from the
        // function signature.
        drain_filter(&mut item.attrs, |attr| match &attr.meta {
            Meta::NameValue(attr) => attr.path == parse_quote!(doc),
            Meta::List(attr) => attr.path == parse_quote!(prop),
            _ => false,
        });
        item.sig.inputs.iter_mut().for_each(|arg| {
            if let FnArg::Typed(ty) = arg {
                drain_filter(&mut ty.attrs, |attr| match &attr.meta {
                    Meta::NameValue(attr) => attr.path == parse_quote!(doc),
                    Meta::List(attr) => attr.path == parse_quote!(prop),
                    _ => false,
                });
            }
        });

        Ok(Self {
            is_transparent: false,
            docs,
            unknown_attrs,
            vis: item.vis.clone(),
            name: convert_from_snake_case(&item.sig.ident),
            props,
            ret: item.sig.output.clone(),
            body: item,
        })
    }
}

impl ToTokens for Model {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            is_transparent,
            docs,
            unknown_attrs,
            vis,
            name,
            props,
            body,
            ret,
        } = self;

        let no_props = props.is_empty();

        // check for components that end ;
        if !is_transparent {
            let ends_semi =
                body.block.stmts.iter().last().and_then(|stmt| match stmt {
                    Stmt::Item(Item::Macro(mac)) => mac.semi_token.as_ref(),
                    _ => None,
                });
            if let Some(semi) = ends_semi {
                proc_macro_error2::emit_error!(
                    semi.span(),
                    "A component that ends with a `view!` macro followed by a \
                     semicolon will return (), an empty view. This is usually \
                     an accident, not intentional, so we prevent it. If you’d \
                     like to return (), you can do it it explicitly by \
                     returning () as the last item from the component."
                );
            }
        }

        #[allow(clippy::redundant_clone)] // false positive
        let body_name = body.sig.ident.clone();

        let (impl_generics, generics, where_clause) =
            body.sig.generics.split_for_impl();

        let props_name = format_ident!("{name}Props");
        let props_builder_name = format_ident!("{name}PropsBuilder");
        #[cfg(feature = "tracing")]
        let trace_name = format!("<{name} />");

        let prop_builder_fields = prop_builder_fields(vis, props);

        let prop_names = prop_names(props);

        let builder_name_doc = LitStr::new(
            &format!(" Props for the [`{name}`] component."),
            name.span(),
        );

        let component_fn_prop_docs = generate_component_fn_prop_docs(props);
        let docs_and_prop_docs = if component_fn_prop_docs.is_empty() {
            // Avoid generating an empty doc line in case the component
            // has no doc and no props.
            quote! {
                #docs
            }
        } else {
            quote! {
                #docs
                #[doc = ""]
                #component_fn_prop_docs
            }
        };

        let (
            tracing_instrument_attr,
            tracing_span_expr,
            tracing_guard_expr,
            tracing_props_expr,
        ) = {
            #[cfg(feature = "tracing")]
            {
                /* TODO for 0.8: fix this
                 *
                 * The problem is that cargo now warns about an
                 * expected "tracing" cfg if you don't have a "tracing"
                 * feature in your actual crate.
                 *
                 * However, until
                 * https://github.com/tokio-rs/tracing/pull/1819 is
                 * merged (?), you can't provide an alternate path for
                 * `tracing` (e.g. ::leptos::tracing), which means that
                 * if you're going to use the macro you *must* have
                 * `tracing` in your Cargo.toml.
                 *
                 * Including the feature-check here causes cargo
                 * warnings on previously-working projects.
                 *
                 * Removing the feature-check here breaks any project
                 * that uses leptos with the tracing feature turned on,
                 * but without a tracing dependency in its Cargo.toml.
                 */
                let instrument = cfg!(feature = "trace-components").then(|| quote! {
                    #[cfg_attr(
                        feature = "tracing",
                        ::leptos::tracing::instrument(level = "info", name = #trace_name, skip_all)
                    )]
                });

                (
                    quote! {
                        #[allow(clippy::let_with_type_underscore)]
                        #instrument
                    },
                    quote! {
                        let __span = ::leptos::tracing::Span::current();
                    },
                    quote! {
                        #[cfg(debug_assertions)]
                        let _guard = __span.entered();
                    },
                    if no_props || !cfg!(feature = "trace-component-props") {
                        quote!()
                    } else {
                        quote! {
                            ::leptos::leptos_dom::tracing_props![#prop_names];
                        }
                    },
                )
            }

            #[cfg(not(feature = "tracing"))]
            {
                (quote!(), quote!(), quote!(), quote!())
            }
        };

        let body_name = unmodified_fn_name_from_fn_name(&body_name);
        let body_expr = quote! {
            #body_name(#prop_names)
        };

        let component = if *is_transparent {
            body_expr
        } else if cfg!(feature = "__internal_erase_components") {
            quote! {
                ::leptos::prelude::IntoMaybeErased::into_maybe_erased(
                    ::leptos::reactive::graph::untrack_with_diagnostics(
                        move || {
                            #tracing_guard_expr
                            #tracing_props_expr
                            #body_expr
                        }
                    )
                )
            }
        } else {
            quote! {
                ::leptos::reactive::graph::untrack_with_diagnostics(
                    move || {
                        #tracing_guard_expr
                        #tracing_props_expr
                        #body_expr
                    }
                )
            }
        };

        let props_arg = if no_props {
            quote! {}
        } else {
            quote! {
                props: #props_name #generics
            }
        };

        let destructure_props = if no_props {
            quote! {}
        } else {
            quote! {
                let #props_name {
                    #prop_names
                } = props;
            }
        };

        let body = quote! {
            #destructure_props
            #tracing_span_expr
            #component
        };

        let output = quote! {
            #[doc = #builder_name_doc]
            #[doc = ""]
            #docs_and_prop_docs
            #[derive(::leptos::typed_builder_macro::TypedBuilder)]
            //#[builder(doc)]
            #[builder(crate_module_path=::leptos::typed_builder)]
            #[allow(non_snake_case)]
            #vis struct #props_name #impl_generics #where_clause {
                #prop_builder_fields
            }

            impl #impl_generics ::leptos::component::Props for #props_name #generics #where_clause {
                type Builder = #props_builder_name #generics;

                fn builder() -> Self::Builder {
                    #props_name::builder()
                }
            }

            #unknown_attrs
            #docs_and_prop_docs
            #[allow(non_snake_case, clippy::too_many_arguments)]
            #[allow(clippy::needless_lifetimes)]
            #tracing_instrument_attr
            #vis fn #name #impl_generics (
                #props_arg
            ) #ret
            #where_clause
            {
                #body
            }
        };

        tokens.append_all(output)
    }
}

impl Model {
    #[allow(clippy::wrong_self_convention)]
    pub fn is_transparent(mut self, is_transparent: bool) -> Self {
        self.is_transparent = is_transparent;
        self
    }
}

/// A model that is more lenient in case of a syntax error in the
/// function body, but does not actually implement the behavior of the
/// real model. Used to improve IDE / rust-analyzer auto-completion in
/// case of a syntax error.
pub struct DummyModel {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub sig: Signature,
    pub body: TokenStream,
}

impl Parse for DummyModel {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = input.call(Attribute::parse_outer)?;
        // Drop unknown attributes like #[deprecated]
        drain_filter(&mut attrs, |attr| {
            !docs::is_lint_attr(attr) && !attr.path().is_ident("doc")
        });

        let vis: Visibility = input.parse()?;
        let mut sig: Signature = input.parse()?;
        maybe_modify_return_type(&mut sig.output);

        // The body is left untouched, so it will not cause an error
        // even if the syntax is invalid.
        let body: TokenStream = input.parse()?;

        Ok(Self {
            attrs,
            vis,
            sig,
            body,
        })
    }
}

impl ToTokens for DummyModel {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            attrs,
            vis,
            sig,
            body,
        } = self;

        // Strip attributes like documentation comments and #[prop] from
        // the signature, so as to not confuse the user with incorrect
        // error messages.
        let sig = {
            let mut sig = sig.clone();
            sig.inputs.iter_mut().for_each(|arg| {
                if let FnArg::Typed(ty) = arg {
                    ty.attrs.retain(|attr| match &attr.meta {
                        Meta::List(list) => list
                            .path
                            .segments
                            .first()
                            .map(|n| n.ident != "prop")
                            .unwrap_or(true),
                        Meta::NameValue(name_value) => name_value
                            .path
                            .segments
                            .first()
                            .map(|n| n.ident != "doc")
                            .unwrap_or(true),
                        _ => true,
                    });
                }
            });
            sig
        };

        let output = quote! {
            #(#attrs)*
            #vis #sig #body
        };

        tokens.append_all(output)
    }
}
