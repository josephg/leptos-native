//! Translation between the renderer's Taffy tree and the CDP DOM/CSS
//! shapes. All functions are generic over the port's [`LayoutBackend`].
//!
//! - Tree → `DOM.Node` (element nodes named by `debug_tag_name`).
//! - Computed `Layout` → `DOM.BoxModel` (content/padding/border/margin
//!   quads, in absolute window coordinates).
//! - Taffy `Style` ⇄ a curated subset of CSS declarations, so the
//!   Styles pane can both display and edit them.

use crate::idmap::{self, DOCUMENT_ID, ROOT_ID};
use renderer::{
    Dimension, Display, FlexDirection, LayoutBackend, LengthPercentage,
    LengthPercentageAuto, NodeId, Style,
};
use serde_json::{json, Value};
use taffy::CompactLength;

// ---------------------------------------------------------------------
// Length formatting / parsing
// ---------------------------------------------------------------------

fn fmt_num(v: f32) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Format a Taffy compact length as a CSS token, or `None` for kinds we
/// don't surface (calc, fr, min/max/fit-content).
fn fmt_compact(c: CompactLength) -> Option<String> {
    match c.tag() {
        CompactLength::AUTO_TAG => Some("auto".into()),
        CompactLength::LENGTH_TAG => Some(format!("{}px", fmt_num(c.value()))),
        CompactLength::PERCENT_TAG => Some(format!("{}%", fmt_num(c.value() * 100.0))),
        _ => None,
    }
}

fn fmt_dim(d: Dimension) -> Option<String> {
    fmt_compact(d.into_raw())
}
fn fmt_lp(d: LengthPercentage) -> String {
    fmt_compact(d.into_raw()).unwrap_or_else(|| "0".into())
}
fn fmt_lpa(d: LengthPercentageAuto) -> Option<String> {
    fmt_compact(d.into_raw())
}

fn parse_dim(s: &str) -> Option<Dimension> {
    let s = s.trim();
    if s == "auto" {
        Some(Dimension::auto())
    } else if let Some(p) = s.strip_suffix('%') {
        p.trim().parse::<f32>().ok().map(|v| Dimension::percent(v / 100.0))
    } else if let Some(px) = s.strip_suffix("px") {
        px.trim().parse::<f32>().ok().map(Dimension::length)
    } else {
        s.parse::<f32>().ok().map(Dimension::length)
    }
}

fn parse_lp(s: &str) -> Option<LengthPercentage> {
    let s = s.trim();
    if let Some(p) = s.strip_suffix('%') {
        p.trim().parse::<f32>().ok().map(|v| LengthPercentage::percent(v / 100.0))
    } else if let Some(px) = s.strip_suffix("px") {
        px.trim().parse::<f32>().ok().map(LengthPercentage::length)
    } else {
        s.parse::<f32>().ok().map(LengthPercentage::length)
    }
}

fn parse_lpa(s: &str) -> Option<LengthPercentageAuto> {
    let s = s.trim();
    if s == "auto" {
        Some(LengthPercentageAuto::auto())
    } else if let Some(p) = s.strip_suffix('%') {
        p.trim().parse::<f32>().ok().map(|v| LengthPercentageAuto::percent(v / 100.0))
    } else if let Some(px) = s.strip_suffix("px") {
        px.trim().parse::<f32>().ok().map(LengthPercentageAuto::length)
    } else {
        s.parse::<f32>().ok().map(LengthPercentageAuto::length)
    }
}

// ---------------------------------------------------------------------
// Style ⇄ CSS declarations (the curated subset)
// ---------------------------------------------------------------------

fn display_str(d: Display) -> &'static str {
    match d {
        Display::Flex => "flex",
        Display::Grid => "grid",
        Display::None => "none",
    }
}

fn flex_dir_str(d: FlexDirection) -> &'static str {
    match d {
        FlexDirection::Row => "row",
        FlexDirection::Column => "column",
        FlexDirection::RowReverse => "row-reverse",
        FlexDirection::ColumnReverse => "column-reverse",
    }
}

/// The editable/displayable declarations for a node, in source order.
pub fn css_decls(style: &Style) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    let mut push = |n: &str, val: String| v.push((n.to_string(), val));

    push("display", display_str(style.display).into());
    push("flex-direction", flex_dir_str(style.flex_direction).into());
    push("flex-grow", fmt_num(style.flex_grow));
    push("flex-shrink", fmt_num(style.flex_shrink));

    if let Some(s) = fmt_dim(style.size.width) {
        push("width", s);
    }
    if let Some(s) = fmt_dim(style.size.height) {
        push("height", s);
    }
    if let Some(s) = fmt_dim(style.min_size.width) {
        push("min-width", s);
    }
    if let Some(s) = fmt_dim(style.min_size.height) {
        push("min-height", s);
    }
    if let Some(s) = fmt_dim(style.max_size.width) {
        push("max-width", s);
    }
    if let Some(s) = fmt_dim(style.max_size.height) {
        push("max-height", s);
    }

    push("padding-top", fmt_lp(style.padding.top));
    push("padding-right", fmt_lp(style.padding.right));
    push("padding-bottom", fmt_lp(style.padding.bottom));
    push("padding-left", fmt_lp(style.padding.left));

    if let Some(s) = fmt_lpa(style.margin.top) {
        push("margin-top", s);
    }
    if let Some(s) = fmt_lpa(style.margin.right) {
        push("margin-right", s);
    }
    if let Some(s) = fmt_lpa(style.margin.bottom) {
        push("margin-bottom", s);
    }
    if let Some(s) = fmt_lpa(style.margin.left) {
        push("margin-left", s);
    }

    push("row-gap", fmt_lp(style.gap.height));
    push("column-gap", fmt_lp(style.gap.width));

    v
}

/// Apply one `name: value` declaration to `style`. Returns `true` if the
/// property was recognised and applied.
fn apply_decl(style: &mut Style, name: &str, value: &str) -> bool {
    match name {
        "display" => {
            style.display = match value {
                "flex" => Display::Flex,
                "grid" => Display::Grid,
                "none" => Display::None,
                _ => return false,
            };
            true
        }
        "flex-direction" => {
            style.flex_direction = match value {
                "row" => FlexDirection::Row,
                "column" => FlexDirection::Column,
                "row-reverse" => FlexDirection::RowReverse,
                "column-reverse" => FlexDirection::ColumnReverse,
                _ => return false,
            };
            true
        }
        "flex-grow" => match value.parse::<f32>() {
            Ok(n) => {
                style.flex_grow = n;
                true
            }
            Err(_) => false,
        },
        "flex-shrink" => match value.parse::<f32>() {
            Ok(n) => {
                style.flex_shrink = n;
                true
            }
            Err(_) => false,
        },
        "width" => set(&mut style.size.width, parse_dim(value)),
        "height" => set(&mut style.size.height, parse_dim(value)),
        "min-width" => set(&mut style.min_size.width, parse_dim(value)),
        "min-height" => set(&mut style.min_size.height, parse_dim(value)),
        "max-width" => set(&mut style.max_size.width, parse_dim(value)),
        "max-height" => set(&mut style.max_size.height, parse_dim(value)),
        "padding-top" => set(&mut style.padding.top, parse_lp(value)),
        "padding-right" => set(&mut style.padding.right, parse_lp(value)),
        "padding-bottom" => set(&mut style.padding.bottom, parse_lp(value)),
        "padding-left" => set(&mut style.padding.left, parse_lp(value)),
        "padding" => match parse_lp(value) {
            Some(p) => {
                style.padding.top = p;
                style.padding.right = p;
                style.padding.bottom = p;
                style.padding.left = p;
                true
            }
            None => false,
        },
        "margin-top" => set(&mut style.margin.top, parse_lpa(value)),
        "margin-right" => set(&mut style.margin.right, parse_lpa(value)),
        "margin-bottom" => set(&mut style.margin.bottom, parse_lpa(value)),
        "margin-left" => set(&mut style.margin.left, parse_lpa(value)),
        "margin" => match parse_lpa(value) {
            Some(m) => {
                style.margin.top = m;
                style.margin.right = m;
                style.margin.bottom = m;
                style.margin.left = m;
                true
            }
            None => false,
        },
        "row-gap" => set(&mut style.gap.height, parse_lp(value)),
        "column-gap" => set(&mut style.gap.width, parse_lp(value)),
        "gap" => match parse_lp(value) {
            Some(g) => {
                style.gap.width = g;
                style.gap.height = g;
                true
            }
            None => false,
        },
        _ => false,
    }
}

fn set<T>(slot: &mut T, parsed: Option<T>) -> bool {
    match parsed {
        Some(v) => {
            *slot = v;
            true
        }
        None => false,
    }
}

/// Parse a full declaration block (`"width: 10px; padding: 8px;"`) onto a
/// clone of the node's current style and write it back, then trigger the
/// port's relayout. No-op if the node is gone.
pub fn apply_css_text<B: LayoutBackend>(
    id: NodeId,
    text: &str,
    schedule: &dyn Fn(NodeId),
) {
    let Some(mut style) = B::style(id) else {
        return;
    };
    for decl in text.split(';') {
        if let Some((name, value)) = decl.split_once(':') {
            apply_decl(&mut style, name.trim(), value.trim());
        }
    }
    B::set_style(id, style);
    schedule(id);
}

// ---------------------------------------------------------------------
// CSS.CSSStyle (with ranges so the Styles pane is editable)
// ---------------------------------------------------------------------

/// Synthetic stylesheet id for a node's inline style.
pub fn sheet_id(cdp_id: i64) -> String {
    format!("inline-{cdp_id}")
}

/// Parse a synthetic stylesheet id back to its CDP node id.
pub fn sheet_node(sheet: &str) -> Option<i64> {
    sheet.strip_prefix("inline-").and_then(|s| s.parse().ok())
}

/// Build a `CSS.CSSStyle` for the node's inline (Taffy) style.
pub fn css_style_json<B: LayoutBackend>(id: NodeId) -> Value {
    let cdp = idmap::cdp_id(id);
    let style = B::style(id).unwrap_or_default();
    let decls = css_decls(&style);

    let mut props = Vec::new();
    let mut text = String::new();
    for (name, value) in &decls {
        let frag = format!("{name}: {value};");
        let start = text.len();
        let end = start + frag.len();
        props.push(json!({
            "name": name,
            "value": value,
            "text": frag,
            "important": false,
            "disabled": false,
            "implicit": false,
            "range": { "startLine": 0, "startColumn": start, "endLine": 0, "endColumn": end },
        }));
        text.push_str(&frag);
        text.push(' ');
    }
    let text_len = text.len();

    json!({
        "styleSheetId": sheet_id(cdp),
        "cssProperties": props,
        "shorthandEntries": [],
        "cssText": text,
        "range": { "startLine": 0, "startColumn": 0, "endLine": 0, "endColumn": text_len },
    })
}

/// `CSS.getComputedStyleForNode` payload: *used* values, like a browser
/// — the box metrics (width/height/padding/border/margin) come from the
/// computed Taffy [`Layout`] in px, not the (often `auto`) style. The
/// non-box properties (display, flex-*) come from the style.
pub fn computed_style_json<B: LayoutBackend>(id: NodeId) -> Vec<Value> {
    let style = B::style(id).unwrap_or_default();
    let mut out: Vec<Value> = Vec::new();
    let mut push = |n: &str, v: String| out.push(json!({ "name": n, "value": v }));

    push("display", display_str(style.display).into());
    push("flex-direction", flex_dir_str(style.flex_direction).into());
    push("flex-grow", fmt_num(style.flex_grow));
    push("flex-shrink", fmt_num(style.flex_shrink));
    // The Computed-pane box-model diagram needs these to interpret the
    // metrics: Taffy sizes are border-box, hence `border-box`.
    push("box-sizing", "border-box".into());
    push(
        "position",
        match style.position {
            renderer::Position::Absolute => "absolute",
            renderer::Position::Relative => "relative",
        }
        .into(),
    );

    let l = B::layout(id).unwrap_or_default();
    let px = |v: f32| format!("{}px", fmt_num(v));
    push("width", px(l.size.width));
    push("height", px(l.size.height));
    push("padding-top", px(l.padding.top));
    push("padding-right", px(l.padding.right));
    push("padding-bottom", px(l.padding.bottom));
    push("padding-left", px(l.padding.left));
    push("border-top-width", px(l.border.top));
    push("border-right-width", px(l.border.right));
    push("border-bottom-width", px(l.border.bottom));
    push("border-left-width", px(l.border.left));
    push("margin-top", px(l.margin.top));
    push("margin-right", px(l.margin.right));
    push("margin-bottom", px(l.margin.bottom));
    push("margin-left", px(l.margin.left));

    out
}

// ---------------------------------------------------------------------
// DOM.Node tree
// ---------------------------------------------------------------------

fn node_name<B: LayoutBackend>(id: NodeId) -> String {
    let tag = B::debug_tag_name(id);
    if tag.is_empty() {
        "node".into()
    } else {
        tag.into()
    }
}

/// A port-supplied callback yielding a node's displayable attributes
/// (e.g. a button's `title`, a label's `value`). Shown in the Elements
/// tree next to the tag.
pub type AttrFn<'a> = &'a dyn Fn(NodeId) -> Vec<(String, String)>;

/// Flatten attribute pairs into CDP's `[name, value, name, value, …]`.
fn attrs_array(pairs: Vec<(String, String)>) -> Vec<String> {
    let mut out = Vec::with_capacity(pairs.len() * 2);
    for (n, v) in pairs {
        out.push(n);
        out.push(v);
    }
    out
}

/// One element node, including children to `depth` (`depth == 0` omits the
/// `children` array but still reports `childNodeCount`).
pub fn node_json<B: LayoutBackend>(id: NodeId, depth: i32, attrs: AttrFn) -> Value {
    let cdp = idmap::cdp_id(id);
    let kids = B::children(id);
    let name = node_name::<B>(id);
    let mut node = json!({
        "nodeId": cdp,
        "backendNodeId": cdp,
        "parentId": B::parent(id).map(idmap::cdp_id),
        "nodeType": 1,
        "nodeName": name.to_uppercase(),
        "localName": name,
        "nodeValue": "",
        "childNodeCount": kids.len(),
        "attributes": attrs_array(attrs(id)),
    });
    if depth != 0 {
        let children: Vec<Value> = kids
            .iter()
            .map(|c| node_json::<B>(*c, depth - 1, attrs))
            .collect();
        node["children"] = json!(children);
    }
    node
}

/// The synthetic `#document` node and its single root-container child.
/// Roots (and their descendants) are expanded to `depth` (a negative
/// value means the entire subtree, matching CDP's convention).
pub fn document_json<B: LayoutBackend>(attrs: AttrFn, depth: i32) -> Value {
    let roots = B::roots();
    let children: Vec<Value> =
        roots.iter().map(|r| node_json::<B>(*r, depth, attrs)).collect();
    let root_el = json!({
        "nodeId": ROOT_ID,
        "backendNodeId": ROOT_ID,
        "parentId": DOCUMENT_ID,
        "nodeType": 1,
        "nodeName": "APP",
        "localName": "app",
        "nodeValue": "",
        "childNodeCount": roots.len(),
        "children": children,
    });
    json!({
        "nodeId": DOCUMENT_ID,
        "backendNodeId": DOCUMENT_ID,
        "nodeType": 9,
        "nodeName": "#document",
        "localName": "",
        "nodeValue": "",
        "documentURL": "leptos://app",
        "baseURL": "leptos://app",
        "childNodeCount": 1,
        "children": [root_el],
    })
}

/// Children of a CDP node (handles the two synthetic ids), each as a
/// depth-1 element node — for `DOM.requestChildNodes` → `setChildNodes`.
pub fn child_nodes_json<B: LayoutBackend>(cdp: i64, attrs: AttrFn) -> Vec<Value> {
    if cdp == DOCUMENT_ID {
        return vec![document_json::<B>(attrs, 1)["children"][0].clone()];
    }
    let kids: Vec<NodeId> = if cdp == ROOT_ID {
        B::roots()
    } else {
        match idmap::taffy(cdp) {
            Some(node) => B::children(node),
            None => return Vec::new(),
        }
    };
    kids.iter().map(|c| node_json::<B>(*c, 1, attrs)).collect()
}

// ---------------------------------------------------------------------
// DOM.BoxModel
// ---------------------------------------------------------------------

/// Absolute (window-space) top-left of a node's border box, summing the
/// relative `location` of the node and all its ancestors.
fn abs_origin<B: LayoutBackend>(id: NodeId) -> (f32, f32) {
    let (mut x, mut y) = (0.0_f32, 0.0_f32);
    let mut cur = Some(id);
    while let Some(c) = cur {
        if let Some(l) = B::layout(c) {
            x += l.location.x;
            y += l.location.y;
        }
        cur = B::parent(c);
    }
    (x, y)
}

/// Clockwise quad (TL, TR, BR, BL) from a rectangle.
fn quad(x: f32, y: f32, w: f32, h: f32) -> Value {
    json!([x, y, x + w, y, x + w, y + h, x, y + h])
}

/// `DOM.BoxModel` from the computed layout. `None` if the node has no
/// stored layout (synthetic nodes, freed nodes).
pub fn box_model_json<B: LayoutBackend>(id: NodeId) -> Option<Value> {
    let l = B::layout(id)?;
    let (bx, by) = abs_origin::<B>(id);
    let (bw, bh) = (l.size.width, l.size.height);

    // border box → padding box (inset by border) → content box (inset
    // further by padding); margin box outsets the border box.
    let border = quad(bx, by, bw, bh);
    let pad = quad(
        bx + l.border.left,
        by + l.border.top,
        bw - l.border.left - l.border.right,
        bh - l.border.top - l.border.bottom,
    );
    let content = quad(
        bx + l.border.left + l.padding.left,
        by + l.border.top + l.padding.top,
        bw - l.border.left - l.border.right - l.padding.left - l.padding.right,
        bh - l.border.top - l.border.bottom - l.padding.top - l.padding.bottom,
    );
    let margin = quad(
        bx - l.margin.left,
        by - l.margin.top,
        bw + l.margin.left + l.margin.right,
        bh + l.margin.top + l.margin.bottom,
    );

    Some(json!({
        "content": content,
        "padding": pad,
        "border": border,
        "margin": margin,
        "width": bw.round() as i64,
        "height": bh.round() as i64,
    }))
}
