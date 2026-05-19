//! `Render<R>` for string types — `&str`, `String`, `Cow<'_, str>`,
//! `Rc<str>`, `Arc<str>`. Each renders as a single text node.

use super::{Mountable, Render};
use crate::layout::TreeRef;
use crate::renderer::{CastFrom, Renderer};
use std::{borrow::Cow, rc::Rc, sync::Arc};

pub struct StrState<'a, R: Renderer> {
    pub(crate) node: R::Text,
    str: &'a str,
}

impl<'a, R: Renderer> Render<R> for &'a str {
    type State = StrState<'a, R>;

    fn build(self, tree: &TreeRef<R::Backend>) -> Self::State {
        StrState { node: R::create_text_node(tree, self), str: self }
    }

    fn rebuild(self, state: &mut Self::State) {
        if self != state.str {
            R::set_text(&state.node, self);
            state.str = self;
        }
    }
}

impl<R: Renderer> Mountable<R> for StrState<'_, R> {
    fn unmount(&mut self) {
        R::remove(self.node.as_ref());
    }
    fn mount(&mut self, parent: &R::Element, marker: Option<&R::Node>) {
        R::insert_node(parent, self.node.as_ref(), marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = R::get_parent(self.node.as_ref())
            .and_then(R::Element::cast_from)
        {
            child.mount(&parent, Some(self.node.as_ref()));
            true
        } else {
            false
        }
    }
    fn elements(&self) -> Vec<R::Element> {
        Vec::new()
    }
}

pub struct StringState<R: Renderer> {
    node: R::Text,
    str: String,
}

impl<R: Renderer> Render<R> for String {
    type State = StringState<R>;
    fn build(self, tree: &TreeRef<R::Backend>) -> Self::State {
        StringState { node: R::create_text_node(tree, &self), str: self }
    }
    fn rebuild(self, state: &mut Self::State) {
        if self != state.str {
            R::set_text(&state.node, &self);
            state.str = self;
        }
    }
}

impl<R: Renderer> Mountable<R> for StringState<R> {
    fn unmount(&mut self) {
        R::remove(self.node.as_ref());
    }
    fn mount(&mut self, parent: &R::Element, marker: Option<&R::Node>) {
        R::insert_node(parent, self.node.as_ref(), marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = R::get_parent(self.node.as_ref())
            .and_then(R::Element::cast_from)
        {
            child.mount(&parent, Some(self.node.as_ref()));
            true
        } else {
            false
        }
    }
    fn elements(&self) -> Vec<R::Element> {
        Vec::new()
    }
}

pub struct CowStrState<'a, R: Renderer> {
    node: R::Text,
    str: Cow<'a, str>,
}

impl<'a, R: Renderer> Render<R> for Cow<'a, str> {
    type State = CowStrState<'a, R>;
    fn build(self, tree: &TreeRef<R::Backend>) -> Self::State {
        CowStrState { node: R::create_text_node(tree, &self), str: self }
    }
    fn rebuild(self, state: &mut Self::State) {
        if self != state.str {
            R::set_text(&state.node, &self);
            state.str = self;
        }
    }
}

impl<R: Renderer> Mountable<R> for CowStrState<'_, R> {
    fn unmount(&mut self) {
        R::remove(self.node.as_ref());
    }
    fn mount(&mut self, parent: &R::Element, marker: Option<&R::Node>) {
        R::insert_node(parent, self.node.as_ref(), marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = R::get_parent(self.node.as_ref())
            .and_then(R::Element::cast_from)
        {
            child.mount(&parent, Some(self.node.as_ref()));
            true
        } else {
            false
        }
    }
    fn elements(&self) -> Vec<R::Element> {
        Vec::new()
    }
}

pub struct RcStrState<R: Renderer> {
    node: R::Text,
    str: Rc<str>,
}

impl<R: Renderer> Render<R> for Rc<str> {
    type State = RcStrState<R>;
    fn build(self, tree: &TreeRef<R::Backend>) -> Self::State {
        RcStrState { node: R::create_text_node(tree, &self), str: self }
    }
    fn rebuild(self, state: &mut Self::State) {
        if !Rc::ptr_eq(&self, &state.str) {
            R::set_text(&state.node, &self);
            state.str = self;
        }
    }
}

impl<R: Renderer> Mountable<R> for RcStrState<R> {
    fn unmount(&mut self) {
        R::remove(self.node.as_ref());
    }
    fn mount(&mut self, parent: &R::Element, marker: Option<&R::Node>) {
        R::insert_node(parent, self.node.as_ref(), marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = R::get_parent(self.node.as_ref())
            .and_then(R::Element::cast_from)
        {
            child.mount(&parent, Some(self.node.as_ref()));
            true
        } else {
            false
        }
    }
    fn elements(&self) -> Vec<R::Element> {
        Vec::new()
    }
}

pub struct ArcStrState<R: Renderer> {
    node: R::Text,
    str: Arc<str>,
}

impl<R: Renderer> Render<R> for Arc<str> {
    type State = ArcStrState<R>;
    fn build(self, tree: &TreeRef<R::Backend>) -> Self::State {
        ArcStrState { node: R::create_text_node(tree, &self), str: self }
    }
    fn rebuild(self, state: &mut Self::State) {
        if !Arc::ptr_eq(&self, &state.str) {
            R::set_text(&state.node, &self);
            state.str = self;
        }
    }
}

impl<R: Renderer> Mountable<R> for ArcStrState<R> {
    fn unmount(&mut self) {
        R::remove(self.node.as_ref());
    }
    fn mount(&mut self, parent: &R::Element, marker: Option<&R::Node>) {
        R::insert_node(parent, self.node.as_ref(), marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = R::get_parent(self.node.as_ref())
            .and_then(R::Element::cast_from)
        {
            child.mount(&parent, Some(self.node.as_ref()));
            true
        } else {
            false
        }
    }
    fn elements(&self) -> Vec<R::Element> {
        Vec::new()
    }
}
