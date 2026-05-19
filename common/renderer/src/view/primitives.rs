//! `Render<R>` for primitive scalar types — bool, char, integers, floats.
//! Each renders as a text node displaying the value's `to_string()`.

use super::{Mountable, Render};
use crate::layout::TreeRef;
use crate::renderer::{CastFrom, Renderer};

/// Retained state for a primitive — the platform Text node plus the last
/// value, so rebuild can skip the platform call when the value is unchanged.
pub struct PrimitiveState<R: Renderer, T> {
    text: R::Text,
    last: T,
}

macro_rules! impl_render_primitive {
    ($($ty:ty),* $(,)?) => {
        $(
            impl<R: Renderer> Render<R> for $ty {
                type State = PrimitiveState<R, $ty>;

                fn build(self, tree: &TreeRef<R::Backend>) -> Self::State {
                    let text = R::create_text_node(tree, &self.to_string());
                    PrimitiveState { text, last: self }
                }

                fn rebuild(self, state: &mut Self::State) {
                    if self != state.last {
                        R::set_text(&state.text, &self.to_string());
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

impl<R: Renderer, T> Mountable<R> for PrimitiveState<R, T> {
    fn unmount(&mut self) {
        R::remove(self.text.as_ref());
    }

    fn mount(&mut self, parent: &R::Element, marker: Option<&R::Node>) {
        R::insert_node(parent, self.text.as_ref(), marker);
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = R::get_parent(self.text.as_ref())
            .and_then(R::Element::cast_from)
        {
            child.mount(&parent, Some(self.text.as_ref()));
            true
        } else {
            false
        }
    }

    fn elements(&self) -> Vec<R::Element> {
        Vec::new()
    }
}
