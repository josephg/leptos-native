//! `Render<R>` for string types — `&str`, `String`, `Cow<'_, str>`,
//! `Rc<str>`, `Arc<str>`. Each renders as a single text node.

use crate::renderer::node::Node;
use super::{Mountable, Render};
use crate::renderer::Backend;
use std::{borrow::Cow, rc::Rc, sync::Arc};

pub struct StrState<'a, R: Backend> {
    pub(crate) node: Node<R>,
    str: &'a str,
}

impl<'a, R: Backend> Render<R> for &'a str {
    type State = StrState<'a, R>;

    fn build(self) -> Self::State {
        StrState { node: R::create_text_node(self), str: self }
    }

    fn rebuild(self, state: &mut Self::State) {
        if self != state.str {
            R::set_text(state.node, self);
            state.str = self;
        }
    }
}

impl<R: Backend> Mountable<R> for StrState<'_, R> {
    fn unmount(&mut self) {
        self.node.remove();
    }
    fn mount(&mut self, parent: Node<R>, marker: Option<Node<R>>) {
        parent.insert_node(self.node, marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = self.node.parent()
        {
            child.mount(parent, Some(self.node));
            true
        } else {
            false
        }
    }
    fn elements(&self) -> Vec<Node<R>> {
        Vec::new()
    }
}

pub struct StringState<R: Backend> {
    node: Node<R>,
    str: String,
}

impl<R: Backend> Render<R> for String {
    type State = StringState<R>;
    fn build(self) -> Self::State {
        StringState { node: R::create_text_node(&self), str: self }
    }
    fn rebuild(self, state: &mut Self::State) {
        if self != state.str {
            R::set_text(state.node, &self);
            state.str = self;
        }
    }
}

impl<R: Backend> Mountable<R> for StringState<R> {
    fn unmount(&mut self) {
        self.node.remove();
    }
    fn mount(&mut self, parent: Node<R>, marker: Option<Node<R>>) {
        parent.insert_node(self.node, marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = self.node.parent()
        {
            child.mount(parent, Some(self.node));
            true
        } else {
            false
        }
    }
    fn elements(&self) -> Vec<Node<R>> {
        Vec::new()
    }
}

pub struct CowStrState<'a, R: Backend> {
    node: Node<R>,
    str: Cow<'a, str>,
}

impl<'a, R: Backend> Render<R> for Cow<'a, str> {
    type State = CowStrState<'a, R>;
    fn build(self) -> Self::State {
        CowStrState { node: R::create_text_node(&self), str: self }
    }
    fn rebuild(self, state: &mut Self::State) {
        if self != state.str {
            R::set_text(state.node, &self);
            state.str = self;
        }
    }
}

impl<R: Backend> Mountable<R> for CowStrState<'_, R> {
    fn unmount(&mut self) {
        self.node.remove();
    }
    fn mount(&mut self, parent: Node<R>, marker: Option<Node<R>>) {
        parent.insert_node(self.node, marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = self.node.parent()
        {
            child.mount(parent, Some(self.node));
            true
        } else {
            false
        }
    }
    fn elements(&self) -> Vec<Node<R>> {
        Vec::new()
    }
}

pub struct RcStrState<R: Backend> {
    node: Node<R>,
    str: Rc<str>,
}

impl<R: Backend> Render<R> for Rc<str> {
    type State = RcStrState<R>;
    fn build(self) -> Self::State {
        RcStrState { node: R::create_text_node(&self), str: self }
    }
    fn rebuild(self, state: &mut Self::State) {
        if !Rc::ptr_eq(&self, &state.str) {
            R::set_text(state.node, &self);
            state.str = self;
        }
    }
}

impl<R: Backend> Mountable<R> for RcStrState<R> {
    fn unmount(&mut self) {
        self.node.remove();
    }
    fn mount(&mut self, parent: Node<R>, marker: Option<Node<R>>) {
        parent.insert_node(self.node, marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = self.node.parent()
        {
            child.mount(parent, Some(self.node));
            true
        } else {
            false
        }
    }
    fn elements(&self) -> Vec<Node<R>> {
        Vec::new()
    }
}

pub struct ArcStrState<R: Backend> {
    node: Node<R>,
    str: Arc<str>,
}

impl<R: Backend> Render<R> for Arc<str> {
    type State = ArcStrState<R>;
    fn build(self) -> Self::State {
        ArcStrState { node: R::create_text_node(&self), str: self }
    }
    fn rebuild(self, state: &mut Self::State) {
        if !Arc::ptr_eq(&self, &state.str) {
            R::set_text(state.node, &self);
            state.str = self;
        }
    }
}

impl<R: Backend> Mountable<R> for ArcStrState<R> {
    fn unmount(&mut self) {
        self.node.remove();
    }
    fn mount(&mut self, parent: Node<R>, marker: Option<Node<R>>) {
        parent.insert_node(self.node, marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = self.node.parent()
        {
            child.mount(parent, Some(self.node));
            true
        } else {
            false
        }
    }
    fn elements(&self) -> Vec<Node<R>> {
        Vec::new()
    }
}
