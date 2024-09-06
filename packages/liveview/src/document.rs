use dioxus_core::ScopeId;
use dioxus_document::{Document, EvalError};
use std::rc::Rc;

use crate::query::{Query, QueryEngine};

/// Provides the LiveviewDocument through [`ScopeId::provide_context`].
pub fn init_eval() {
    let query = ScopeId::ROOT.consume_context::<QueryEngine>().unwrap();
    let provider: Rc<dyn Document> = Rc::new(LiveviewDocument { query });
    ScopeId::ROOT.provide_context(provider);
}

/// Reprints the liveview-target's provider of evaluators.
pub struct LiveviewDocument {
    query: QueryEngine,
}

impl Document for LiveviewDocument {
    fn eval(&self, js: String) -> dioxus_document::Eval {
        todo!()
    }

    fn set_title(&self, title: String) {
        _ = self.eval(format!("window.document.title = '{}';", title));
    }

    fn create_head_element(
        &self,
        name: &str,
        attributes: Vec<(&str, String)>,
        contents: Option<String>,
    ) {
        todo!()
    }

    fn current_route(&self) -> String {
        todo!()
    }

    fn go_back(&self) {
        todo!()
    }

    fn go_forward(&self) {
        todo!()
    }

    fn push_route(&self, route: String) {
        todo!()
    }

    fn replace_route(&self, path: String) {
        todo!()
    }
}
