//! Per-connection CDP command dispatcher.
//!
//! Turns one incoming JSON-RPC message into the outgoing response (and
//! any events) to write back. Node-id mapping is global (see
//! [`crate::idmap`]); the session only carries the port [`Hooks`].
//! Generic over the port's [`Backend`]; every tree access runs
//! synchronously on the main thread, since the whole server future is
//! driven by the port's main-loop executor.

use crate::idmap;
use crate::mapping;
use crate::Hooks;
use leptos_native::renderer::{Backend, NodeId};
use serde::Deserialize;
use serde_json::{json, Value};
use std::marker::PhantomData;

#[derive(Deserialize)]
struct Command {
    id: Option<i64>,
    method: String,
    #[serde(default)]
    params: Value,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
}

pub struct Session<B: Backend> {
    hooks: Hooks,
    _pd: PhantomData<B>,
}

impl<B: Backend> Session<B> {
    pub fn new(hooks: Hooks) -> Self {
        Session {
            hooks,
            _pd: PhantomData,
        }
    }

    /// Handle one JSON-RPC message; returns the JSON strings to send back
    /// (a response, possibly preceded by events). An unparseable message
    /// yields nothing.
    pub fn dispatch(&mut self, raw: &str) -> Vec<String> {
        let Ok(cmd) = serde_json::from_str::<Command>(raw) else {
            eprintln!("devtools: unparseable message: {raw}");
            return Vec::new();
        };
        let mut events: Vec<Value> = Vec::new();
        let result = self.handle(&cmd.method, &cmd.params, &mut events);

        let sid = cmd.session_id.as_deref();
        let mut out: Vec<String> = events
            .into_iter()
            .map(|mut e| {
                if let (Some(s), Value::Object(m)) = (sid, &mut e) {
                    m.insert("sessionId".into(), json!(s));
                }
                e.to_string()
            })
            .collect();

        if let Some(id) = cmd.id {
            let mut msg = match result {
                Ok(r) => json!({ "id": id, "result": r }),
                Err(message) => json!({ "id": id, "error": { "code": -32000, "message": message } }),
            };
            if let (Some(s), Value::Object(m)) = (sid, &mut msg) {
                m.insert("sessionId".into(), json!(s));
            }
            out.push(msg.to_string());
        }
        out
    }

    fn node_id(&self, params: &Value) -> Option<NodeId> {
        idmap::taffy(params.get("nodeId").and_then(Value::as_i64)?)
    }

    fn handle(
        &mut self,
        method: &str,
        params: &Value,
        events: &mut Vec<Value>,
    ) -> Result<Value, String> {
        let attrs = self.hooks.node_attributes.as_ref();
        match method {
            // --- DOM ----------------------------------------------------
            "DOM.getDocument" => {
                // CDP convention: negative depth = whole tree. We default
                // to that so the Elements tree populates in one round trip.
                let depth = params.get("depth").and_then(Value::as_i64).unwrap_or(-1) as i32;
                Ok(json!({ "root": mapping::document_json::<B>(attrs, depth) }))
            }
            "DOM.requestChildNodes" => {
                if let Some(cdp) = params.get("nodeId").and_then(Value::as_i64) {
                    let nodes = mapping::child_nodes_json::<B>(cdp, attrs);
                    events.push(json!({
                        "method": "DOM.setChildNodes",
                        "params": { "parentId": cdp, "nodes": nodes },
                    }));
                }
                Ok(json!({}))
            }
            "DOM.getBoxModel" => {
                let id = self.node_id(params).ok_or("unknown node")?;
                let model = mapping::box_model_json::<B>(id).ok_or("no layout for node")?;
                Ok(json!({ "model": model }))
            }
            "DOM.describeNode" => {
                let id = self.node_id(params).ok_or("unknown node")?;
                Ok(json!({ "node": mapping::node_json::<B>(id, 0, attrs) }))
            }
            "DOM.pushNodesByBackendIdsToFrontend" => {
                let ids = params
                    .get("backendNodeIds")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                Ok(json!({ "nodeIds": ids }))
            }
            "DOM.enable" | "DOM.disable" | "DOM.setInspectedNode"
            | "DOM.setAttributeValue" => Ok(json!({})),

            // --- CSS ----------------------------------------------------
            "CSS.enable" | "CSS.disable" => Ok(json!({})),
            "CSS.getComputedStyleForNode" => {
                let id = self.node_id(params).ok_or("unknown node")?;
                Ok(json!({ "computedStyle": mapping::computed_style_json::<B>(id) }))
            }
            "CSS.getInlineStylesForNode" => {
                let id = self.node_id(params).ok_or("unknown node")?;
                Ok(json!({ "inlineStyle": mapping::css_style_json::<B>(id) }))
            }
            "CSS.getMatchedStylesForNode" => {
                let id = self.node_id(params).ok_or("unknown node")?;
                Ok(json!({
                    "inlineStyle": mapping::css_style_json::<B>(id),
                    "matchedCSSRules": [],
                    "pseudoElements": [],
                    "inherited": [],
                    "cssKeyframesRules": [],
                }))
            }
            "CSS.setStyleTexts" => {
                let edits = params
                    .get("edits")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let mut styles = Vec::new();
                for edit in edits {
                    let sheet = edit.get("styleSheetId").and_then(Value::as_str).unwrap_or("");
                    let text = edit.get("text").and_then(Value::as_str).unwrap_or("");
                    if let Some(cdp) = mapping::sheet_node(sheet) {
                        if let Some(id) = idmap::taffy(cdp) {
                            mapping::apply_css_text::<B>(
                                id,
                                text,
                                self.hooks.schedule_relayout.as_ref(),
                            );
                            styles.push(mapping::css_style_json::<B>(id));
                        }
                    }
                }
                Ok(json!({ "styles": styles }))
            }

            // --- Overlay -------------------------------------------------
            "Overlay.highlightNode" => {
                (self.hooks.set_highlight)(self.node_id(params));
                Ok(json!({}))
            }
            "Overlay.hideHighlight" | "Overlay.highlightRect" => {
                (self.hooks.set_highlight)(None);
                Ok(json!({}))
            }
            // Inspect-from-app: the frontend enables "pick an element"
            // mode; the port watches its pointer and reports back via
            // `crate::notify_node_*` events.
            "Overlay.setInspectMode" => {
                let on = params
                    .get("mode")
                    .and_then(Value::as_str)
                    .map(|m| m != "none")
                    .unwrap_or(false);
                (self.hooks.set_inspect_mode)(on);
                Ok(json!({}))
            }

            // --- stubs the frontend needs during attach ----------------
            "Page.enable" | "Page.disable" => Ok(json!({})),
            "Page.getResourceTree" => Ok(json!({
                "frameTree": {
                    "frame": {
                        "id": "leptos-frame",
                        "loaderId": "leptos-loader",
                        "url": "leptos://app",
                        "domainAndRegistry": "",
                        "securityOrigin": "leptos://app",
                        "mimeType": "text/html",
                        "secureContextType": "Secure",
                        "crossOriginIsolatedContextType": "NotIsolated",
                        "gatedAPIFeatures": [],
                    },
                    "resources": [],
                }
            })),
            "Runtime.enable" => {
                events.push(json!({
                    "method": "Runtime.executionContextCreated",
                    "params": {
                        "context": {
                            "id": 1,
                            "origin": "leptos://app",
                            "name": "leptos-native",
                            "uniqueId": "leptos-native-1",
                            "auxData": { "isDefault": true, "frameId": "leptos-frame" },
                        }
                    },
                }));
                Ok(json!({}))
            }

            // Everything else: ack with an empty result. DevTools tolerates
            // unknown-but-successful commands; this keeps the panel alive.
            _ => Ok(json!({})),
        }
    }
}
