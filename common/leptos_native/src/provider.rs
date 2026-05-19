use crate::{children::TypedChildren, into_view::IntoView};
use leptos_macro::component;
use reactive_graph::owner::{provide_context, Owner};
use renderer::{reactive_graph::OwnedView, renderer::Renderer};

/// Uses the context API to [`provide_context`] to its children and
/// descendants, without overwriting any contexts of the same type in
/// its own reactive scope.
///
/// This prevents issues related to "context shadowing."
#[component]
pub fn Provider<T, Chil, R>(
    /// The value to be provided via context.
    value: T,
    children: TypedChildren<Chil, R>,
) -> impl IntoView<R>
where
    R: Renderer,
    T: Send + Sync + 'static,
    Chil: IntoView<R> + 'static,
{
    let owner = Owner::current()
        .expect("no current reactive Owner found")
        .child();
    let children = children.into_inner();
    let children = owner.with(|| {
        provide_context(value);
        children()
    });
    OwnedView::new_with_owner(children, owner)
}
