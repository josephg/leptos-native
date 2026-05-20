//! `Render<R>` impls for collections — `Vec<T>` (unkeyed diff) and
//! fixed-size `[T; N]` arrays. Keyed iteration lives in `keyed.rs`.

use crate::{
    renderer::Renderer,
    view::{Mountable, Render},
};

impl<R, T> Render<R> for Vec<T>
where
    R: Renderer,
    T: Render<R>,
{
    type State = VecState<T::State, R>;

    fn build(self) -> Self::State {
        let marker = R::create_placeholder();
        VecState {
            states: self.into_iter().map(|v| v.build()).collect(),
            marker,
        }
    }

    fn rebuild(self, state: &mut Self::State) {
        let VecState { states, marker } = state;
        let old = states;
        // unkeyed diff
        if old.is_empty() {
            let mut new_states: Vec<T::State> =
                self.into_iter().map(|v| v.build()).collect();
            for item in new_states.iter_mut() {
                R::try_mount_before(item, &*marker);
            }
            *old = new_states;
        } else if self.is_empty() {
            for item in old.iter_mut() {
                item.unmount();
            }
            old.clear();
        } else {
            let mut adds = vec![];
            let mut removes_at_end = 0;
            let mut new_iter = self.into_iter();
            let mut old_iter = old.iter_mut();
            loop {
                match (new_iter.next(), old_iter.next()) {
                    (Some(new), Some(old_state)) => {
                        T::rebuild(new, old_state)
                    }
                    (Some(new), None) => {
                        let mut new_state = new.build();
                        R::try_mount_before(&mut new_state, &*marker);
                        adds.push(new_state);
                    }
                    (None, Some(old_state)) => {
                        removes_at_end += 1;
                        old_state.unmount();
                    }
                    (None, None) => break,
                }
            }
            // drain remaining new items (the iterator chain above stops at
            // None/None but `new_iter` may still hold items past Some/None
            // pairings — push all of them as adds)
            for new in new_iter {
                let mut new_state = new.build();
                R::try_mount_before(&mut new_state, &*marker);
                adds.push(new_state);
            }
            for old_state in old_iter {
                removes_at_end += 1;
                old_state.unmount();
            }
            old.truncate(old.len() - removes_at_end);
            old.append(&mut adds);
        }
    }
}

/// Retained view state for a `Vec<_>`.
pub struct VecState<T, R>
where
    R: Renderer,
    T: Mountable<R>,
{
    states: Vec<T>,
    /// Marker placeholder so new items can be inserted-before a known node
    /// rather than appended-after the last known item.
    marker: R::Node,
}

impl<R, T> Mountable<R> for VecState<T, R>
where
    R: Renderer,
    T: Mountable<R>,
{
    fn unmount(&mut self) {
        for state in self.states.iter_mut() {
            state.unmount();
        }
        self.marker.unmount();
    }

    fn mount(&mut self, parent: &R::Node, marker: Option<&R::Node>) {
        for state in self.states.iter_mut() {
            state.mount(parent, marker);
        }
        self.marker.mount(parent, marker);
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        for state in &self.states {
            if state.insert_before_this(child) {
                return true;
            }
        }
        self.marker.insert_before_this(child)
    }

    fn elements(&self) -> Vec<R::Node> {
        self.states
            .iter()
            .flat_map(|item| item.elements())
            .collect()
    }
}
