use crate::renderer::node::Node;
use crate::renderer::{
    Backend,
    view::{AddAnyAttr, ApplyAttr, Mountable, Render},
};
use reactive_graph::owner::Owner;

/// A view wrapper that sets the reactive [`Owner`] to a particular owner
/// whenever it is rendered.
#[derive(Debug, Clone)]
pub struct OwnedView<T> {
    owner: Owner,
    view: T,
}

impl<T> OwnedView<T> {
    /// Wraps a view with the current owner.
    pub fn new(view: T) -> Self {
        let owner = Owner::current().expect("no reactive owner");
        Self { owner, view }
    }

    /// Wraps a view with the given owner.
    pub fn new_with_owner(view: T, owner: Owner) -> Self {
        Self { owner, view }
    }
}

/// Retained view state for an [`OwnedView`].
#[derive(Debug, Clone)]
pub struct OwnedViewState<T, R>
where
    T: Mountable<R>,
    R: Backend,
{
    owner: Owner,
    state: T,
    rndr: std::marker::PhantomData<R>,
}

impl<T, R> OwnedViewState<T, R>
where
    T: Mountable<R>,
    R: Backend,
{
    fn new(state: T, owner: Owner) -> Self {
        Self {
            owner,
            state,
            rndr: std::marker::PhantomData,
        }
    }
}

impl<T, R> Render<R> for OwnedView<T>
where
    R: Backend,
    T: Render<R>,
{
    type State = OwnedViewState<T::State, R>;

    fn build(self) -> Self::State {
        let state = self.owner.with(|| self.view.build());
        OwnedViewState::new(state, self.owner)
    }

    fn rebuild(self, state: &mut Self::State) {
        let OwnedView { owner, view, .. } = self;
        owner.with(|| view.rebuild(&mut state.state));
        state.owner = owner;
    }
}

impl<T, R> Mountable<R> for OwnedViewState<T, R>
where
    R: Backend,
    T: Mountable<R>,
{
    fn unmount(&mut self) {
        self.state.unmount();
    }

    fn mount(&mut self, parent: Node<R>, marker: Option<Node<R>>) {
        self.state.mount(parent, marker);
    }

    fn try_mount(
        &mut self,
        parent: Node<R>,
        marker: Option<Node<R>>,
    ) -> bool {
        self.state.try_mount(parent, marker)
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        self.state.insert_before_this(child)
    }

    fn elements(&self) -> Vec<Node<R>> {
        self.state.elements()
    }
}

/// `OwnedView<T>` forwards `add_any_attr` to its wrapped view. The
/// reactive owner doesn't change anything about the spread path.
impl<T, R> AddAnyAttr<R> for OwnedView<T>
where
    R: Backend,
    T: AddAnyAttr<R>,
{
    fn add_any_attr<A: ApplyAttr<R>>(self, attr: A) -> Self {
        let OwnedView { owner, view } = self;
        OwnedView {
            owner,
            view: view.add_any_attr(attr),
        }
    }
}
