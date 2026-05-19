//! Prop-level types + the helpers that turn props into TypedBuilder
//! fields and component-fn doc lines.

use super::{
    docs::Docs,
    util::{is_option, unwrap_option},
};
use attribute_derive::FromAttr;
use proc_macro2::{Ident, TokenStream};
use proc_macro_error2::abort;
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{
    parse_quote, spanned::Spanned, FnArg, LitStr, Pat, PatIdent, Type,
    Visibility,
};

pub(super) struct Prop {
    pub(super) docs: Docs,
    pub(super) prop_opts: PropOpt,
    pub(super) name: PatIdent,
    pub(super) ty: Type,
}

impl Prop {
    pub(super) fn new(arg: FnArg) -> Self {
        let typed = if let FnArg::Typed(ty) = arg {
            ty
        } else {
            abort!(arg, "receiver not allowed in `fn`");
        };

        let prop_opts =
            PropOpt::from_attributes(&typed.attrs).unwrap_or_else(|e| {
                // TODO: replace with `.unwrap_or_abort()` once
                // https://gitlab.com/CreepySkeleton/proc-macro-error/-/issues/17
                // is fixed
                abort!(e.span(), e.to_string());
            });

        let name = match *typed.pat {
            Pat::Ident(i) => {
                if let Some(name) = &prop_opts.name {
                    PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: None,
                        ident: Ident::new(name, i.span()),
                        subpat: None,
                    }
                } else {
                    i
                }
            }
            Pat::Struct(_) | Pat::Tuple(_) | Pat::TupleStruct(_) => {
                if let Some(name) = &prop_opts.name {
                    PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: None,
                        ident: Ident::new(name, typed.pat.span()),
                        subpat: None,
                    }
                } else {
                    abort!(
                        typed.pat,
                        "destructured props must be given a name e.g. \
                         #[prop(name = \"data\")]"
                    );
                }
            }
            _ => {
                abort!(
                    typed.pat,
                    "only `prop: bool` style types are allowed within the \
                     `#[component]` macro"
                );
            }
        };

        Self {
            docs: Docs::new(&typed.attrs),
            prop_opts,
            name,
            ty: *typed.ty,
        }
    }
}

#[derive(Clone, Debug, FromAttr)]
#[attribute(ident = prop)]
pub(super) struct PropOpt {
    #[attribute(conflicts = [optional_no_strip, strip_option])]
    pub optional: bool,
    #[attribute(conflicts = [optional, strip_option])]
    pub optional_no_strip: bool,
    #[attribute(conflicts = [optional, optional_no_strip])]
    pub strip_option: bool,
    #[attribute(example = "5 * 10")]
    pub default: Option<syn::Expr>,
    pub into: bool,
    pub attrs: bool,
    pub name: Option<String>,
}

struct TypedBuilderOpts<'a> {
    default: bool,
    default_with_value: Option<syn::Expr>,
    strip_option: bool,
    into: bool,
    ty: &'a Type,
}

impl<'a> TypedBuilderOpts<'a> {
    fn from_opts(opts: &PropOpt, ty: &'a Type) -> Self {
        Self {
            default: opts.optional || opts.optional_no_strip || opts.attrs,
            default_with_value: opts.default.clone(),
            strip_option: opts.strip_option || opts.optional && is_option(ty),
            into: opts.into,
            ty,
        }
    }
}

impl ToTokens for TypedBuilderOpts<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let default = if let Some(v) = &self.default_with_value {
            let v = v.to_token_stream().to_string();
            quote! { default_code=#v, }
        } else if self.default {
            quote! { default, }
        } else {
            quote! {}
        };

        // If self.strip_option && self.into, strip_option is
        // represented as part of the transform closure.
        let strip_option = if self.strip_option && !self.into {
            quote! { strip_option, }
        } else {
            quote! {}
        };

        let into = if self.into {
            if !self.strip_option {
                let ty = &self.ty;
                quote! {
                    fn transform<__IntoReactiveValueMarker>(value: impl ::leptos_native::prelude::IntoReactiveValue<#ty, __IntoReactiveValueMarker>) -> #ty {
                        value.into_reactive_value()
                    },
                }
            } else {
                let ty = unwrap_option(self.ty);
                quote! {
                    fn transform<__IntoReactiveValueMarker>(value: impl ::leptos_native::prelude::IntoReactiveValue<#ty, __IntoReactiveValueMarker>) -> Option<#ty> {
                        Some(value.into_reactive_value())
                    },
                }
            }
        } else {
            quote! {}
        };

        let setter = if !strip_option.is_empty() || !into.is_empty() {
            quote! { setter(#strip_option #into) }
        } else {
            quote! {}
        };

        let output = if !default.is_empty() || !setter.is_empty() {
            quote! { #[builder(#default #setter)] }
        } else {
            quote! {}
        };

        tokens.append_all(output);
    }
}

pub(super) fn prop_builder_fields(
    vis: &Visibility,
    props: &[Prop],
) -> TokenStream {
    props
        .iter()
        .map(|prop| {
            let Prop {
                docs,
                name,
                prop_opts,
                ty,
            } = prop;

            let builder_attrs = TypedBuilderOpts::from_opts(prop_opts, ty);

            let builder_docs = prop_to_doc(prop, PropDocStyle::Inline);

            // Children won't need documentation in many cases
            let allow_missing_docs = if name.ident == "children" {
                quote!(#[allow(missing_docs)])
            } else {
                quote!()
            };

            let PatIdent { ident, by_ref, .. } = &name;

            quote! {
                #docs
                #builder_docs
                #builder_attrs
                #allow_missing_docs
                #vis #by_ref #ident: #ty,
            }
        })
        .collect()
}

pub(super) fn prop_names(props: &[Prop]) -> TokenStream {
    props
        .iter()
        .map(|Prop { name, .. }| {
            // fields like mutability are removed because unneeded in
            // the contexts in which this is used
            let ident = &name.ident;
            quote! { #ident, }
        })
        .collect()
}

pub(super) fn generate_component_fn_prop_docs(
    props: &[Prop],
) -> TokenStream {
    let required_prop_docs = props
        .iter()
        .filter(|Prop { prop_opts, .. }| {
            !(prop_opts.optional
                || prop_opts.optional_no_strip
                || prop_opts.default.is_some())
        })
        .map(|p| prop_to_doc(p, PropDocStyle::List))
        .collect::<TokenStream>();

    let optional_prop_docs = props
        .iter()
        .filter(|Prop { prop_opts, .. }| {
            prop_opts.optional
                || prop_opts.optional_no_strip
                || prop_opts.default.is_some()
        })
        .map(|p| prop_to_doc(p, PropDocStyle::List))
        .collect::<TokenStream>();

    let required_prop_docs = if !required_prop_docs.is_empty() {
        quote! {
            #[doc = " # Required Props"]
            #required_prop_docs
        }
    } else {
        quote! {}
    };

    let optional_prop_docs = if !optional_prop_docs.is_empty() {
        quote! {
            #[doc = " # Optional Props"]
            #optional_prop_docs
        }
    } else {
        quote! {}
    };

    quote! {
        #required_prop_docs
        #optional_prop_docs
    }
}

#[derive(Clone, Copy)]
enum PropDocStyle {
    List,
    Inline,
}

fn prop_to_doc(
    Prop {
        docs,
        name,
        ty,
        prop_opts,
    }: &Prop,
    style: PropDocStyle,
) -> TokenStream {
    let ty = if (prop_opts.optional || prop_opts.strip_option) && is_option(ty)
    {
        unwrap_option(ty)
    } else {
        ty.to_owned()
    };

    let type_item: syn::Item = parse_quote! {
        type SomeType = #ty;
    };

    let file = syn::File {
        shebang: None,
        attrs: vec![],
        items: vec![type_item],
    };

    let pretty_ty = prettyplease::unparse(&file);

    let pretty_ty = &pretty_ty[16..&pretty_ty.len() - 2];

    match style {
        PropDocStyle::List => {
            let arg_ty_doc = LitStr::new(
                &if !prop_opts.into {
                    format!(" - **{}**: [`{pretty_ty}`]", quote!(#name))
                } else {
                    format!(
                        " - **{}**: [`impl Into<{pretty_ty}>`]({pretty_ty})",
                        quote!(#name),
                    )
                },
                name.ident.span(),
            );

            let arg_user_docs = docs.padded();

            quote! {
                #[doc = #arg_ty_doc]
                #arg_user_docs
            }
        }
        PropDocStyle::Inline => {
            let arg_ty_doc = LitStr::new(
                &if !prop_opts.into {
                    format!(
                        "**{}**: [`{}`]{}",
                        quote!(#name),
                        pretty_ty,
                        docs.typed_builder()
                    )
                } else {
                    format!(
                        "**{}**: `impl`[`Into<{}>`]{}",
                        quote!(#name),
                        pretty_ty,
                        docs.typed_builder()
                    )
                },
                name.ident.span(),
            );

            quote! {
                #[builder(setter(doc = #arg_ty_doc))]
            }
        }
    }
}
