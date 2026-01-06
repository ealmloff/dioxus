//! Web-only bindings using standard wasm-bindgen instead of sledgehammer.
//!
//! This module provides the `Interpreter` type for web-only builds.
//! For binary-protocol (desktop/liveview), see `unified_bindings.rs`.

use wasm_bindgen::prelude::*;
use web_sys::Node;

#[wasm_bindgen(module = "/src/js/interpreter.js")]
extern "C" {
    pub type Interpreter;

    #[wasm_bindgen(constructor)]
    pub fn new() -> Interpreter;

    // BaseInterpreter methods
    #[wasm_bindgen(method)]
    pub fn initialize(this: &Interpreter, root: Node, handler: &js_sys::Function);

    #[wasm_bindgen(method, js_name = "saveTemplate")]
    pub fn save_template(this: &Interpreter, nodes: Vec<Node>, tmpl_id: u16);

    #[wasm_bindgen(method)]
    pub fn hydrate(this: &Interpreter, ids: Vec<u32>, under: Vec<Node>);

    #[wasm_bindgen(method, js_name = "getNode")]
    pub fn get_node(this: &Interpreter, id: u32) -> Node;

    #[wasm_bindgen(method, js_name = "pushRoot")]
    pub fn push_root_node(this: &Interpreter, node: Node);

    // DOM mutation methods
    #[wasm_bindgen(method, js_name = "pushRootById")]
    pub fn push_root(this: &Interpreter, root: u32);

    #[wasm_bindgen(method, js_name = "appendChildrenToNode")]
    pub fn append_children(this: &Interpreter, id: u32, many: u16);

    #[wasm_bindgen(method, js_name = "popRoot")]
    pub fn pop_root(this: &Interpreter);

    #[wasm_bindgen(method, js_name = "replaceWith")]
    pub fn replace_with(this: &Interpreter, id: u32, n: u16);

    #[wasm_bindgen(method, js_name = "insertAfter")]
    pub fn insert_after(this: &Interpreter, id: u32, n: u16);

    #[wasm_bindgen(method, js_name = "insertBefore")]
    pub fn insert_before(this: &Interpreter, id: u32, n: u16);

    #[wasm_bindgen(method, js_name = "removeNode")]
    pub fn remove(this: &Interpreter, id: u32);

    #[wasm_bindgen(method, js_name = "createRawText")]
    pub fn create_raw_text(this: &Interpreter, text: &str);

    #[wasm_bindgen(method, js_name = "createTextNodeWithId")]
    pub fn create_text_node(this: &Interpreter, text: &str, id: u32);

    #[wasm_bindgen(method, js_name = "createPlaceholder")]
    pub fn create_placeholder(this: &Interpreter, id: u32);

    #[wasm_bindgen(method, js_name = "newEventListener")]
    pub fn new_event_listener(this: &Interpreter, event_name: &str, id: u32, bubbles: bool);

    #[wasm_bindgen(method, js_name = "removeEventListenerById")]
    pub fn remove_event_listener(this: &Interpreter, event_name: &str, id: u32, bubbles: bool);

    #[wasm_bindgen(method, js_name = "setText")]
    pub fn set_text(this: &Interpreter, id: u32, text: &str);

    #[wasm_bindgen(method, js_name = "setAttributeById")]
    pub fn set_attribute(this: &Interpreter, id: u32, field: &str, value: &str, ns: Option<&str>);

    #[wasm_bindgen(method, js_name = "removeAttributeById")]
    pub fn remove_attribute(this: &Interpreter, id: u32, field: &str, ns: Option<&str>);

    #[wasm_bindgen(method, js_name = "assignIdByPath")]
    pub fn assign_id(this: &Interpreter, path: &[u8], id: u32);

    #[wasm_bindgen(method, js_name = "replacePlaceholderByPath")]
    pub fn replace_placeholder(this: &Interpreter, path: &[u8], n: u16);

    #[wasm_bindgen(method, js_name = "loadTemplateById")]
    pub fn load_template(this: &Interpreter, tmpl_id: u16, index: u16, id: u32);
}

impl Default for Interpreter {
    fn default() -> Self {
        Interpreter::new()
    }
}

impl Interpreter {
    /// No-op flush for API compatibility - web executes immediately
    pub fn flush(&self) {
        // No-op: wasm-bindgen calls execute immediately
    }

    /// Returns self for compatibility with existing code that calls interpreter.base()
    pub fn base(&self) -> &Interpreter {
        self
    }
}
