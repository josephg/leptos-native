//! `ViewSpec` — a renderer-agnostic AST describing a randomly-
//! generated cocoa view tree. The fuzzer generates a `Node`, mounts
//! it with reactive bindings driven by [`SignalStore`], runs chaos
//! mutations on those signals, then asks the spec to emit a *static*
//! tree from the final signal values and compares both mounted
//! NSView hierarchies.

use std::fmt;

pub type SignalId = u32;

/// Attribute value: either baked-in (`Static`) or driven by a
/// signal in [`crate::signals::SignalStore`] (`Reactive`).
#[derive(Clone, Debug)]
pub enum Attr<T> {
    Static(T),
    Reactive { id: SignalId, initial: T },
}

impl<T: Clone> Attr<T> {
    pub fn initial(&self) -> &T {
        match self {
            Attr::Static(v) | Attr::Reactive { initial: v, .. } => v,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerKind {
    VStack,
    HStack,
    View,
    /// `<scroll_view>`. The renderer ensures bounded sizing by
    /// wrapping its content in a fixed-height vstack inside the
    /// generator (see `gen_node` placement rules).
    ScrollView,
}

impl ContainerKind {
    pub fn tag(self) -> &'static str {
        match self {
            ContainerKind::VStack => "vstack",
            ContainerKind::HStack => "hstack",
            ContainerKind::View => "view",
            ContainerKind::ScrollView => "scroll_view",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Node {
    Container {
        kind: ContainerKind,
        padding: Option<Attr<f32>>,
        gap: Option<Attr<f32>>,
        children: Vec<Node>,
    },
    Button {
        title: Attr<String>,
        enabled: Option<Attr<bool>>,
        hidden: Option<Attr<bool>>,
    },
    Label {
        text: Attr<String>,
        hidden: Option<Attr<bool>>,
    },
    Checkbox {
        title: Attr<String>,
        checked: Option<Attr<bool>>,
    },
    TextField {
        value: Attr<String>,
        placeholder: Option<Attr<String>>,
        enabled: Option<Attr<bool>>,
        /// If true, render via `secure_text_field()`
        /// (NSSecureTextField). Distinct cocoa builder constructor
        /// — both render to the same Node type so the comparison
        /// works the same.
        secure: bool,
    },
    /// Multi-line text view (NSTextView inside NSScrollView).
    /// Bind:value reads/writes the underlying string.
    TextView {
        value: Attr<String>,
        enabled: Option<Attr<bool>>,
    },
    /// NSSlider — 0..1 value, optionally with explicit range.
    Slider {
        value: Attr<f64>,
        enabled: Option<Attr<bool>>,
        vertical: bool,
    },
    /// NSStepper — integer-ish increment/decrement on a value.
    Stepper {
        value: Attr<f64>,
        enabled: Option<Attr<bool>>,
    },
    /// NSProgressIndicator — value 0..max_value plus an
    /// indeterminate flag.
    ProgressIndicator {
        value: Attr<f64>,
        indeterminate: Option<Attr<bool>>,
    },
    /// NSPopUpButton — a list of item labels (static) and a
    /// reactive selection index.
    PopUpButton {
        items: Vec<String>,
        selection: Attr<usize>,
        enabled: Option<Attr<bool>>,
    },
    /// NSSegmentedControl — same shape as popup but rendered as a
    /// horizontal segmented control.
    SegmentedControl {
        items: Vec<String>,
        selection: Attr<usize>,
        enabled: Option<Attr<bool>>,
    },
    /// NSDatePicker — value is the current date. We never mutate
    /// it in chaos (no reasonable random date dispatch) but it's
    /// included for structural / mount/unmount coverage.
    DatePicker {
        enabled: Option<Attr<bool>>,
    },
    /// NSColorWell — same story: static color, mount coverage
    /// only.
    ColorWell {
        enabled: Option<Attr<bool>>,
    },
    /// NSImageView via an SF Symbol name (no on-disk dependency).
    ImageView {
        sf_symbol: String,
    },
    /// Conditional: when `when` is true render `on`, else render
    /// `off` (or nothing, if `off` is None). Toggling `when` via
    /// the chaos loop changes the *shape* of the mounted tree,
    /// not just attribute values — stresses the unmount/re-mount
    /// path on the inactive branch.
    Show {
        when: Attr<bool>,
        on: Box<Node>,
        off: Option<Box<Node>>,
    },
}

impl Node {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Node::Container { kind, .. } => kind.tag(),
            Node::Button { .. } => "button",
            Node::Label { .. } => "label",
            Node::Checkbox { .. } => "checkbox",
            Node::TextField { secure: true, .. } => "secure_text_field",
            Node::TextField { .. } => "text_field",
            Node::TextView { .. } => "text_view",
            Node::Slider { .. } => "slider",
            Node::Stepper { .. } => "stepper",
            Node::ProgressIndicator { .. } => "progress_indicator",
            Node::PopUpButton { .. } => "pop_up_button",
            Node::SegmentedControl { .. } => "segmented_control",
            Node::DatePicker { .. } => "date_picker",
            Node::ColorWell { .. } => "color_well",
            Node::ImageView { .. } => "image_view",
            Node::Show { .. } => "show",
        }
    }

    /// Count nodes (including self). Walks both Show branches —
    /// useful for spec-size diagnostics even though only one is
    /// mounted at a time.
    pub fn size(&self) -> usize {
        let inner: usize = match self {
            Node::Container { children, .. } => {
                children.iter().map(Node::size).sum()
            }
            Node::Show { on, off, .. } => {
                on.size() + off.as_ref().map_or(0, |n| n.size())
            }
            _ => 0,
        };
        1 + inner
    }

    /// Whether this leaf may host any children. Used by the
    /// generator to decide where to nest deeper trees.
    pub fn is_container(&self) -> bool {
        matches!(self, Node::Container { .. })
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_tree(self, f, 0)
    }
}

fn write_tree(node: &Node, f: &mut fmt::Formatter, indent: usize) -> fmt::Result {
    let pad = "  ".repeat(indent);
    match node {
        Node::Container { kind, children, .. } => {
            writeln!(f, "{pad}{} ({} children)", kind.tag(), children.len())?;
            for c in children {
                write_tree(c, f, indent + 1)?;
            }
        }
        Node::Button { title, .. } => {
            writeln!(f, "{pad}button title={:?}", title.initial())?;
        }
        Node::Label { text, .. } => {
            writeln!(f, "{pad}label text={:?}", text.initial())?;
        }
        Node::Checkbox { title, checked } => {
            writeln!(
                f,
                "{pad}checkbox title={:?} checked={:?}",
                title.initial(),
                checked.as_ref().map(|c| c.initial()),
            )?;
        }
        Node::TextField { value, secure, .. } => {
            writeln!(
                f,
                "{pad}{}{} value={:?}",
                if *secure { "secure_" } else { "" },
                "text_field",
                value.initial()
            )?;
        }
        Node::TextView { value, .. } => {
            writeln!(f, "{pad}text_view value={:?}", value.initial())?;
        }
        Node::Slider { value, .. } => {
            writeln!(f, "{pad}slider value={:?}", value.initial())?;
        }
        Node::Stepper { value, .. } => {
            writeln!(f, "{pad}stepper value={:?}", value.initial())?;
        }
        Node::ProgressIndicator { value, .. } => {
            writeln!(f, "{pad}progress_indicator value={:?}", value.initial())?;
        }
        Node::PopUpButton { items, selection, .. } => {
            writeln!(
                f,
                "{pad}pop_up_button items={:?} sel={:?}",
                items,
                selection.initial()
            )?;
        }
        Node::SegmentedControl { items, selection, .. } => {
            writeln!(
                f,
                "{pad}segmented_control items={:?} sel={:?}",
                items,
                selection.initial()
            )?;
        }
        Node::DatePicker { .. } => writeln!(f, "{pad}date_picker")?,
        Node::ColorWell { .. } => writeln!(f, "{pad}color_well")?,
        Node::ImageView { sf_symbol } => {
            writeln!(f, "{pad}image_view sf_symbol={:?}", sf_symbol)?
        }
        Node::Show { when, on, off } => {
            writeln!(
                f,
                "{pad}show when={:?} {}",
                when.initial(),
                if off.is_some() { "(+fallback)" } else { "" }
            )?;
            writeln!(f, "{pad}  on:")?;
            write_tree(on, f, indent + 2)?;
            if let Some(off) = off {
                writeln!(f, "{pad}  off:")?;
                write_tree(off, f, indent + 2)?;
            }
        }
    }
    Ok(())
}
