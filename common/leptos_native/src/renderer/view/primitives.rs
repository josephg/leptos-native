//! `Render<R>` for primitive scalar types — bool, char, integers, floats.
//! Each renders as a text node displaying the value's `to_string()`.

use crate::renderer::node::Node;
use super::{Mountable, Render};
use crate::renderer::Backend;

/// Retained state for a primitive — the platform Text node plus the last
/// value, so rebuild can skip the platform call when the value is unchanged.
pub struct PrimitiveState<R: Backend, T> {
    text: Node<R>,
    last: T,
}

macro_rules! impl_render_primitive {
    ($($ty:ty),* $(,)?) => {
        $(
            impl<R: Backend> Render<R> for $ty {
                type State = PrimitiveState<R, $ty>;

                fn build(self) -> Self::State {
                    let text = R::create_text_node(&self.to_string());
                    PrimitiveState { text, last: self }
                }

                fn rebuild(self, state: &mut Self::State) {
                    if self != state.last {
                        R::set_text(state.text, &self.to_string());
                        state.last = self;
                    }
                }
            }
        )*
    };
}

impl_render_primitive!(
    bool, char, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize,
    f32, f64,
);

impl<R: Backend, T> Mountable<R> for PrimitiveState<R, T> {
    fn unmount(&mut self) {
        self.text.remove();
    }

    fn mount(&mut self, parent: Node<R>, marker: Option<Node<R>>) {
        parent.insert_node(self.text, marker);
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = self.text.parent() {
            child.mount(parent, Some(self.text));
            true
        } else {
            false
        }
    }

    fn elements(&self) -> Vec<Node<R>> {
        Vec::new()
    }
}
