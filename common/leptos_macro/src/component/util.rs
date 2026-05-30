//! Type-level helpers used across the `#[component]` macro.

use convert_case::{Case::Snake, Casing};
use proc_macro2::{Ident, Span};
use proc_macro_error2::abort;
use syn::{
    spanned::Spanned, token::Colon, visit_mut::VisitMut,
    AngleBracketedGenericArguments, Attribute, GenericArgument, GenericParam,
    Path, PathArguments, ReturnType, Signature, Type, TypeImplTrait, TypeParam,
    TypePath,
};

pub(super) fn maybe_modify_return_type(ret: &mut ReturnType) {
    let _ = ret;
}

/// Manual implementation of `Vec::drain_filter` (which is nightly-only).
/// Removes every element for which `some_predicate` returns `true`.
pub fn drain_filter<T>(
    vec: &mut Vec<T>,
    mut some_predicate: impl FnMut(&mut T) -> bool,
) {
    let mut i = 0;
    while i < vec.len() {
        if some_predicate(&mut vec[i]) {
            _ = vec.remove(i);
        } else {
            i += 1;
        }
    }
}

/// Convert a snake_case identifier to PascalCase, leaving names that
/// are already PascalCase unchanged.
pub fn convert_from_snake_case(name: &Ident) -> Ident {
    use convert_case::Case::Pascal;
    use convert_case_extras::is_case;
    let name_str = name.to_string();
    if !is_case(&name_str, Snake) {
        name.clone()
    } else {
        Ident::new(&name_str.to_case(Pascal), name.span())
    }
}

/// Returns `true` when `ty` is exactly `Option<…>`.
pub fn is_option(ty: &Type) -> bool {
    if let Type::Path(TypePath {
        path: Path { segments, .. },
        ..
    }) = ty
    {
        if let [first] = &segments.iter().collect::<Vec<_>>()[..] {
            first.ident == "Option"
        } else {
            false
        }
    } else {
        false
    }
}

/// Strip `Option<…>` from a type, returning the inner type. Aborts
/// with a help message if `ty` isn't an `Option`.
pub fn unwrap_option(ty: &Type) -> Type {
    const STD_OPTION_MSG: &str =
        "make sure you're not shadowing the `std::option::Option` type that \
         is automatically imported from the standard prelude";

    if let Type::Path(TypePath {
        path: Path { segments, .. },
        ..
    }) = ty
    {
        if let [first] = &segments.iter().collect::<Vec<_>>()[..] {
            if first.ident == "Option" {
                if let PathArguments::AngleBracketed(
                    AngleBracketedGenericArguments { args, .. },
                ) = &first.arguments
                {
                    if let [GenericArgument::Type(ty)] =
                        &args.iter().collect::<Vec<_>>()[..]
                    {
                        return ty.clone();
                    }
                }
            }
        }
    }

    abort!(
        ty,
        "`Option` must be `std::option::Option`";
        help = STD_OPTION_MSG
    );
}

/// Mirror of the user-facing component fn name into the actual
/// implementation function. The macro emits two functions per
/// component: a public wrapper and a hidden body. This is the body's
/// name.
pub fn unmodified_fn_name_from_fn_name(ident: &Ident) -> Ident {
    Ident::new(
        &format!("__component_{}", ident.to_string().to_case(Snake)),
        ident.span(),
    )
}

/// Converts all `impl Trait`s in a function signature to use generic
/// params instead.
pub(super) fn convert_impl_trait_to_generic(sig: &mut Signature) {
    fn new_generic_ident(i: usize, span: Span) -> Ident {
        Ident::new(&format!("__ImplTrait{i}"), span)
    }

    // First: visit all `impl Trait`s and replace them with new generic
    // params.
    #[derive(Default)]
    struct RemoveImplTrait(Vec<TypeImplTrait>);
    impl VisitMut for RemoveImplTrait {
        fn visit_type_mut(&mut self, ty: &mut Type) {
            syn::visit_mut::visit_type_mut(self, ty);
            if matches!(ty, Type::ImplTrait(_)) {
                let ident = new_generic_ident(self.0.len(), ty.span());
                let generic_type = Type::Path(TypePath {
                    qself: None,
                    path: Path::from(ident),
                });
                let Type::ImplTrait(impl_trait) =
                    std::mem::replace(ty, generic_type)
                else {
                    unreachable!();
                };
                self.0.push(impl_trait);
            }
        }

        // Early exits.
        fn visit_attribute_mut(&mut self, _: &mut Attribute) {}
        fn visit_pat_mut(&mut self, _: &mut syn::Pat) {}
    }
    let mut visitor = RemoveImplTrait::default();
    for fn_arg in sig.inputs.iter_mut() {
        visitor.visit_fn_arg_mut(fn_arg);
    }
    let RemoveImplTrait(impl_traits) = visitor;

    // Second: Add the new generic params into the signature.
    for (i, impl_trait) in impl_traits.into_iter().enumerate() {
        let span = impl_trait.span();
        let ident = new_generic_ident(i, span);
        // We can simply append to the end (only lifetime params must
        // be first). Default generics are currently not allowed in
        // `fn`, so this is fine.
        sig.generics.params.push(GenericParam::Type(TypeParam {
            attrs: vec![],
            ident,
            colon_token: Some(Colon { spans: [span] }),
            bounds: impl_trait.bounds,
            eq_token: None,
            default: None,
        }));
    }
}
