//! Spec → cocoa view. Two entry points:
//!
//! - [`build_reactive`] wires every `Attr::Reactive { id, .. }` to
//!   an `RwSignal` in the supplied [`SignalStore`] (creating it
//!   on first reference), so subsequent signal mutations drive the
//!   already-mounted view.
//! - [`build_static`] ignores the reactive bit and uses
//!   `initial` everywhere. Used to construct the comparison tree
//!   *after* the chaos loop, when we re-snapshot the store and
//!   want a from-scratch render of the same final state. Pass the
//!   snapshot via [`crate::signals::SignalStore::snapshot`] and
//!   apply with [`SignalStore::restore`] before calling
//!   `build_static` if you want the final-state values baked in.
//!
//! Children come back as `Vec<AnyView<Dom>>` so containers can hold
//! a dynamic number of unknown-shape descendants.

use crate::signals::SignalStore;
use crate::spec::{Attr, ContainerKind, Node};
use leptos_cocoa::cocoa::bind::BindAttribute;
use leptos_cocoa::cocoa::element::{
    button, checkbox, color_well, date_picker, hstack, image_view,
    label, pop_up_button, progress_indicator, scroll_view,
    secure_text_field, segmented_control, slider, stack, stepper,
    text_field, text_view, vstack, Button, Checkbox, ColorWell, DatePicker
    , ImageView, Label, PopUpButton, ProgressIndicator, ScrollView,
    SegmentedControl, Slider, Stack, Stepper, TextField, TextView,
};
use leptos_cocoa::dom::Date;
use leptos_cocoa::{attr, CocoaBackend};
use reactive_graph::traits::Get;
// `hidden` / `padding` come from the renderer's shared layout-attr
// trait; `into_any` from the renderer's view erasure trait.
use leptos_native::renderer::attrs::WithLayout;
use leptos_native::renderer::view::{AnyView, IntoAny};

/// Either-branch helper: pick the static or reactive path and call
/// the builder method exactly once, with the right concrete type
/// for `IntoMaybeReactive`. Returns the modified builder.
macro_rules! set_string_attr {
    ($builder:expr, $method:ident, $store:expr, $attr:expr) => {{
        match $attr {
            Attr::Static(v) => $builder.$method(v.clone()),
            Attr::Reactive { id, initial } => {
                let sig = $store.ensure_string(*id, initial);
                $builder.$method(move || sig.get())
            }
        }
    }};
}

macro_rules! set_bool_attr {
    ($builder:expr, $method:ident, $store:expr, $attr:expr) => {{
        match $attr {
            Attr::Static(v) => $builder.$method(*v),
            Attr::Reactive { id, initial } => {
                let sig = $store.ensure_bool(*id, *initial);
                $builder.$method(move || sig.get())
            }
        }
    }};
}

macro_rules! set_f32_attr {
    ($builder:expr, $method:ident, $store:expr, $attr:expr) => {{
        match $attr {
            Attr::Static(v) => $builder.$method(*v),
            Attr::Reactive { id, initial } => {
                let sig = $store.ensure_float(*id, *initial);
                $builder.$method(move || sig.get())
            }
        }
    }};
}

/// Edges-typed variant: padding takes `Edges`, not `f32`, so the
/// reactive closure has to wrap the read with `Edges::all(...)`
/// (there's an `IntoMaybeReactive<Edges> for f32` but only for the
/// static branch).
macro_rules! set_padding_attr {
    ($builder:expr, $store:expr, $attr:expr) => {{
        match $attr {
            Attr::Static(v) => $builder.padding(*v),
            Attr::Reactive { id, initial } => {
                let sig = $store.ensure_float(*id, *initial);
                $builder.padding(move || {
                    leptos_native::renderer::attrs::Edges::all(sig.get())
                })
            }
        }
    }};
}

/// Render a node into a type-erased cocoa view, optionally
/// installing reactive bindings into `store`.
pub fn build(node: &Node, store: &SignalStore) -> AnyView<CocoaBackend> {
    match node {
        Node::Container { kind, padding, gap, children } => {
            // Build children up-front so we can pass them as a Vec.
            let kids: Vec<AnyView<CocoaBackend>> =
                children.iter().map(|c| build(c, store)).collect();

            // Stack-shaped container ctor (vstack/hstack/view).
            macro_rules! finish_stack {
                ($ctor:ident) => {{
                    let mut s: Stack<()> = $ctor();
                    if let Some(p) = padding {
                        s = set_padding_attr!(s, store, p);
                    }
                    if let Some(g) = gap {
                        s = set_f32_attr!(s, gap, store, g);
                    }
                    s.child(kids).into_any()
                }};
            }
            match kind {
                ContainerKind::VStack => finish_stack!(vstack),
                ContainerKind::HStack => finish_stack!(hstack),
                ContainerKind::View => finish_stack!(stack),
                ContainerKind::ScrollView => {
                    // ScrollView needs a bounded parent on the
                    // scroll axis. Wrap it in a fixed-height
                    // vstack so layout converges deterministically
                    // regardless of where the spec places it.
                    let mut sv: ScrollView<()> = scroll_view();
                    if let Some(p) = padding {
                        sv = set_padding_attr!(sv, store, p);
                    }
                    let sv = sv.child(kids).height(160.0);
                    sv.into_any()
                }
            }
        }
        Node::Button { title, enabled, hidden } => {
            let mut b: Button = button();
            b = set_string_attr!(b, title, store, title);
            if let Some(e) = enabled {
                b = set_bool_attr!(b, enabled, store, e);
            }
            if let Some(h) = hidden {
                b = set_bool_attr!(b, hidden, store, h);
            }
            b.into_any()
        }
        Node::Label { text, hidden } => {
            let mut l: Label = label();
            l = set_string_attr!(l, text, store, text);
            if let Some(h) = hidden {
                l = set_bool_attr!(l, hidden, store, h);
            }
            l.into_any()
        }
        Node::Checkbox { title, checked } => {
            let mut c: Checkbox = checkbox();
            c = set_string_attr!(c, title, store, title);
            if let Some(ch) = checked {
                // Use `bind:checked` for the Reactive case so
                // user clicks (in --xcui mode) feed back to the
                // signal. Static stays one-way.
                c = match ch {
                    Attr::Static(v) => c.checked(*v),
                    Attr::Reactive { id, initial } => {
                        let sig = store.ensure_bool(*id, *initial);
                        c.bind(attr::Checked, sig)
                    }
                };
            }
            c.into_any()
        }
        Node::TextField { value, placeholder, enabled, secure } => {
            let mut t: TextField =
                if *secure { secure_text_field() } else { text_field() };
            t = match value {
                Attr::Static(v) => t.value(v.clone()),
                Attr::Reactive { id, initial } => {
                    let sig = store.ensure_string(*id, initial);
                    // bind:value so typing in --xcui mode writes
                    // back to the signal.
                    t.bind(attr::Value, sig)
                }
            };
            if let Some(p) = placeholder {
                t = set_string_attr!(t, placeholder, store, p);
            }
            if let Some(e) = enabled {
                t = set_bool_attr!(t, enabled, store, e);
            }
            t.into_any()
        }
        Node::TextView { value, enabled } => {
            let mut t: TextView = text_view();
            t = match value {
                Attr::Static(v) => t.value(v.clone()),
                Attr::Reactive { id, initial } => {
                    let sig = store.ensure_string(*id, initial);
                    t.bind(attr::Value, sig)
                }
            };
            if let Some(e) = enabled {
                t = set_bool_attr!(t, enabled, store, e);
            }
            // text_view tends to be content-sized vertically;
            // give it a fixed-ish bound so layout converges.
            t.height(80.0).into_any()
        }
        Node::Slider { value, enabled, vertical } => {
            let mut s: Slider = slider();
            if *vertical {
                s = s.vertical(true);
            }
            s = match value {
                Attr::Static(v) => s.value(*v),
                Attr::Reactive { id, initial } => {
                    let sig = store.ensure_float64(*id, *initial);
                    s.bind(attr::Value, sig)
                }
            };
            if let Some(e) = enabled {
                s = set_bool_attr!(s, enabled, store, e);
            }
            s.into_any()
        }
        Node::Stepper { value, enabled } => {
            let mut s: Stepper = stepper();
            s = match value {
                Attr::Static(v) => s.value(*v),
                Attr::Reactive { id, initial } => {
                    let sig = store.ensure_float64(*id, *initial);
                    s.bind(attr::Value, sig)
                }
            };
            if let Some(e) = enabled {
                s = set_bool_attr!(s, enabled, store, e);
            }
            s.into_any()
        }
        Node::ProgressIndicator { value, indeterminate } => {
            let mut p: ProgressIndicator = progress_indicator();
            p = match value {
                Attr::Static(v) => p.value(*v),
                Attr::Reactive { id, initial } => {
                    let sig = store.ensure_float64(*id, *initial);
                    // ProgressIndicator's value is one-way only
                    // (no `bind:value` impl). Use a reactive
                    // closure instead.
                    p.value(move || sig.get())
                }
            };
            if let Some(ind) = indeterminate {
                p = set_bool_attr!(p, indeterminate, store, ind);
            }
            p.into_any()
        }
        Node::PopUpButton { items, selection, enabled } => {
            let mut pp: PopUpButton = pop_up_button().items(items.clone());
            pp = match selection {
                Attr::Static(v) => pp.selection(*v),
                Attr::Reactive { id, initial } => {
                    let sig = store.ensure_index(*id, *initial);
                    pp.bind(attr::Value, sig)
                }
            };
            if let Some(e) = enabled {
                pp = set_bool_attr!(pp, enabled, store, e);
            }
            pp.into_any()
        }
        Node::SegmentedControl { items, selection, enabled } => {
            let mut sc: SegmentedControl =
                segmented_control().items(items.clone());
            sc = match selection {
                Attr::Static(v) => sc.selection(*v),
                Attr::Reactive { id, initial } => {
                    let sig = store.ensure_index(*id, *initial);
                    sc.bind(attr::Value, sig)
                }
            };
            if let Some(e) = enabled {
                sc = set_bool_attr!(sc, enabled, store, e);
            }
            sc.into_any()
        }
        Node::DatePicker { enabled } => {
            // Use a fixed epoch-based date so the static rebuild
            // matches the reactive mount exactly (the builder's
            // default is `Date::now()` which drifts by seconds
            // between the two builds and the comparison reads
            // NSDatePicker.stringValue).
            let fixed = Date::from_unix_secs(1_700_000_000.0);
            let mut dp: DatePicker = date_picker().value(fixed);
            if let Some(e) = enabled {
                dp = set_bool_attr!(dp, enabled, store, e);
            }
            dp.into_any()
        }
        Node::ColorWell { enabled } => {
            let mut cw: ColorWell = color_well();
            if let Some(e) = enabled {
                cw = set_bool_attr!(cw, enabled, store, e);
            }
            cw.into_any()
        }
        Node::ImageView { sf_symbol } => {
            let iv: ImageView = image_view().sf_symbol(sf_symbol.clone());
            iv.into_any()
        }
        Node::Show { when, on, off } => {
            // Pre-register the `when` signal so the closure can
            // capture it cheaply (RwSignal: Copy). The on/off
            // sub-trees are rebuilt on each toggle inside the
            // closure — that's the same shape Show would have
            // taken if we went through ShowProps.
            let when_sig = match when {
                Attr::Static(_) => {
                    // Generator should have forced Reactive, but
                    // be defensive: a static condition is
                    // effectively "always render on" — emit on
                    // unconditionally.
                    if let Attr::Static(v) = when {
                        if *v {
                            return build(on, store);
                        } else if let Some(off) = off {
                            return build(off, store);
                        } else {
                            return ().into_any();
                        }
                    }
                    unreachable!()
                }
                Attr::Reactive { id, initial } => {
                    store.ensure_bool(*id, *initial)
                }
            };

            // Move clones of the sub-tree specs + the store into
            // a reactive closure. Each toggle reruns build()
            // against the *current* signal store, so existing
            // signals are reused (ensure_* lookups) and inactive
            // branches drop fully (the closure's previous return
            // is discarded by tachys' reactive update path).
            //
            // Both branches are erased to AnyView so the closure
            // has a single concrete return type the reactive
            // bridge can wrap in a `RenderEffect`.
            let on_spec = (**on).clone();
            let off_spec = off.as_ref().map(|b| (**b).clone());
            let store_for_closure = store.clone();
            let closure = move || -> AnyView<CocoaBackend> {
                let store = &store_for_closure;
                if when_sig.get() {
                    build(&on_spec, store)
                } else {
                    match &off_spec {
                        Some(off) => build(off, store),
                        // `EmptyBranch` builds a real UnitState
                        // placeholder; using `()` here returns a
                        // no-op mountable with no anchor, and
                        // AnyView::rebuild silently drops the new
                        // state when the empty branch flips back
                        // to populated. (Same bug class the
                        // framework `<Show>` had for its
                        // no-fallback case.)
                        None => leptos_cocoa::core::control_flow::EmptyBranch
                            .into_any(),
                    }
                }
            };
            AnyView::new(closure)
        }
        Node::DynamicList { count, max, template } => {
            // Bulk-rebuild list driven by a usize signal. Every
            // chaos write to `count_sig` re-runs the closure,
            // which produces a fresh vstack whose child vector
            // has the current length. AnyView::rebuild then
            // unmounts the old vstack and mounts the new one.
            //
            // Stresses the same code paths Show does but with a
            // wider variety of resulting tree shapes (0..=max
            // children, all the same template).
            let count_sig = match count {
                Attr::Static(v) => {
                    // Static dynamic-list is just a static vstack.
                    let kids: Vec<AnyView<CocoaBackend>> = (0..*v)
                        .map(|_| build(template, store))
                        .collect();
                    return vstack().child(kids).into_any();
                }
                Attr::Reactive { id, initial } => {
                    store.ensure_index(*id, *initial)
                }
            };
            let template = (**template).clone();
            let store = store.clone();
            let max = *max;
            let closure = move || -> AnyView<CocoaBackend> {
                let n = count_sig.get().min(max);
                let kids: Vec<AnyView<CocoaBackend>> = (0..n)
                    .map(|_| build(&template, &store))
                    .collect();
                vstack().child(kids).into_any()
            };
            AnyView::new(closure)
        }
        Node::Grid { children, .. } => {
            // Grid<((), Vec<AnyView<Dom>>)>: !Send (one of its
            // private fields' types breaks auto-Send), and Grid's
            // `.child` accumulates into a tuple type, so
            // dynamic-arity placement-bearing children aren't
            // expressible via the public builder. Fall back to a
            // plain vstack for grid specs — the cells still get
            // rendered, just without grid-solver coverage. The
            // spec variant is left in place so the generator's
            // recursion shape stays stable for repro reproducibility;
            // covering grid layout properly is its own audit item.
            // (Render the children as a stack to at least exercise
            // the recursive mount path.)
            let kids: Vec<AnyView<CocoaBackend>> = children
                .iter()
                .map(|(_, _, kid_spec)| build(kid_spec, store))
                .collect();
            vstack().child(kids).into_any()
        }
    }
}
