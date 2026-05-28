use crate::{
    driver::{Driver, ElementRect, RootContext, TestElement},
    result::TesterError,
};
use dioxus_core::{Element, VirtualDom, consume_context, spawn, use_hook};
use dioxus_desktop::{Config, WindowBuilder, testing::DesktopTestHarness};
use std::{
    cell::RefCell,
    fmt,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, SyncSender},
    },
    time::Duration,
};
use tokio::sync::mpsc as tokio_mpsc;
use wry_bindgen::{JsCast, wasm_bindgen};

const ROOT_ID: &str = "main";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

type ContextFn = Box<dyn FnOnce(&mut VirtualDom) + Send>;

/// A driver backed by the normal desktop renderer and real webview DOM.
///
/// The desktop renderer owns rendering, event wiring, and mutation delivery. This driver only sends
/// commands into the renderer runtime to query or interact with the browser DOM.
pub struct WebSysDriver {
    root: Option<fn() -> Element>,
    contexts: Vec<ContextFn>,
    window_size: Option<(u32, u32)>,
    harness: RefCell<Option<DesktopTestHarness>>,
    command_tx: Option<tokio_mpsc::UnboundedSender<WebCommand>>,
}

impl Driver for WebSysDriver {
    type Selector = WebSelector;
    type ElementId = WebElementId;
    type Element<'driver> = WebSysElement<'driver>;

    fn from_element(element: fn() -> Element) -> Self {
        Self {
            root: Some(element),
            contexts: Vec::new(),
            window_size: None,
            harness: RefCell::new(None),
            command_tx: None,
        }
    }

    fn with_window_size(&mut self, width: u32, height: u32) {
        self.window_size = Some((width, height));
    }

    fn build(&mut self) {
        self.ensure_harness();
        self.request(WebCommand::Build)
            .expect("desktop webview driver failed to build");
    }

    async fn pump(&mut self) -> Result<(), TesterError> {
        self.request(WebCommand::Pump)
    }

    async fn advance_time(&mut self, _duration: Duration) {
        // The desktop backend reads layout from the live webview. There is no synthetic Blitz clock
        // to advance here.
    }

    fn parse_selector(&self, selector: &str) -> Result<Self::Selector, TesterError> {
        self.request(|tx| WebCommand::ParseSelector {
            selector: selector.to_string(),
            tx,
        })?;
        Ok(WebSelector {
            selector: selector.to_string(),
        })
    }

    fn root(&self) -> Self::Element<'_> {
        WebSysElement {
            driver: self,
            id: WebElementId(0),
        }
    }

    fn get_element(&self, selector: &Self::Selector) -> Option<Self::ElementId> {
        self.request(|tx| WebCommand::QueryOne {
            selector: selector.selector.clone(),
            tx,
        })
        .unwrap_or(None)
    }

    fn get_elements(&self, selector: &Self::Selector) -> Vec<Self::ElementId> {
        self.request(|tx| WebCommand::QueryAll {
            selector: selector.selector.clone(),
            tx,
        })
        .unwrap_or_default()
    }

    fn build_element(&self, id: Self::ElementId) -> Self::Element<'_> {
        WebSysElement { driver: self, id }
    }
}

impl WebSysDriver {
    fn ensure_harness(&mut self) {
        if self.harness.borrow().is_some() {
            return;
        }

        let root = self
            .root
            .take()
            .expect("desktop webview driver can only build once");
        let contexts = std::mem::take(&mut self.contexts);
        let window_size = self.window_size.unwrap_or((500, 800));
        let (command_tx, command_rx) = tokio_mpsc::unbounded_channel();
        self.command_tx = Some(command_tx);

        let test_context = DesktopTestContext {
            root,
            command_rx: Arc::new(Mutex::new(Some(command_rx))),
        };
        let make_dom = move || {
            let mut dom = VirtualDom::new(desktop_test_root);
            dom.provide_root_context(test_context);
            for context in contexts {
                context(&mut dom);
            }
            dom
        };
        let window = WindowBuilder::new().with_visible(false).with_inner_size(
            dioxus_desktop::LogicalSize::new(f64::from(window_size.0), f64::from(window_size.1)),
        );
        let config = Config::new()
            .with_window(window)
            .with_root_name(ROOT_ID)
            .with_disable_context_menu(true);

        *self.harness.borrow_mut() = Some(DesktopTestHarness::new(make_dom, config));
    }

    fn request<T>(&self, make_command: impl FnOnce(SyncSender<T>) -> WebCommand) -> T
    where
        T: Send + 'static,
    {
        let (tx, rx) = mpsc::sync_channel(1);
        let command = make_command(tx);
        self.command_tx
            .as_ref()
            .expect("desktop webview driver must be built before it is used")
            .send(command)
            .expect("desktop webview driver command loop stopped");
        self.run_until_response(rx)
    }

    fn run_until_response<T>(&self, rx: Receiver<T>) -> T
    where
        T: Send + 'static,
    {
        let mut harness = self.harness.borrow_mut();
        let harness = harness
            .as_mut()
            .expect("desktop webview driver must be built before it is used");
        harness
            .run_until(REQUEST_TIMEOUT, || rx.try_recv().ok())
            .expect("timed out waiting for desktop webview driver response")
    }

    fn click(&self, id: WebElementId) {
        self.request(|tx| WebCommand::Click { id, tx })
            .expect("desktop webview click failed");
    }

    fn html(&self, id: WebElementId, outer: bool) -> String {
        self.request(|tx| WebCommand::Html { id, outer, tx })
            .expect("desktop webview HTML read failed")
    }

    fn rect(&self, id: WebElementId) -> ElementRect {
        self.request(|tx| WebCommand::Rect { id, tx })
            .expect("desktop webview rect read failed")
    }
}

impl<T: Clone + Send + 'static> RootContext<T> for WebSysDriver {
    fn with_root_context(&mut self, context: T) {
        self.contexts.push(Box::new(move |dom| {
            dom.provide_root_context(context);
        }));
    }
}

/// A parsed CSS selector for the desktop webview backend.
pub struct WebSelector {
    selector: String,
}

/// Stable handle id for an element retained by the desktop webview backend.
#[derive(Debug, Clone, Copy)]
pub struct WebElementId(usize);

/// Element handle returned by [WebSysDriver].
pub struct WebSysElement<'driver> {
    driver: &'driver WebSysDriver,
    id: WebElementId,
}

impl fmt::Debug for WebSysElement<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebSysElement")
            .field("id", &self.id)
            .finish()
    }
}

impl TestElement for WebSysElement<'_> {
    fn click(&self) {
        self.driver.click(self.id);
    }

    fn outer_html(&self) -> String {
        self.driver.html(self.id, true)
    }

    fn inner_html(&self) -> String {
        self.driver.html(self.id, false)
    }

    fn bounding_rect(&self) -> ElementRect {
        self.driver.rect(self.id)
    }
}

#[derive(Clone)]
struct DesktopTestContext {
    root: fn() -> Element,
    command_rx: Arc<Mutex<Option<tokio_mpsc::UnboundedReceiver<WebCommand>>>>,
}

fn desktop_test_root() -> Element {
    let context: DesktopTestContext = consume_context();
    let root = context.root;

    use_hook(move || {
        let Some(command_rx) = context.command_rx.lock().ok().and_then(|mut rx| rx.take()) else {
            return;
        };
        spawn(run_desktop_commands(command_rx));
    });

    root()
}

enum WebCommand {
    Build(SyncSender<Result<(), TesterError>>),
    Pump(SyncSender<Result<(), TesterError>>),
    ParseSelector {
        selector: String,
        tx: SyncSender<Result<(), TesterError>>,
    },
    QueryOne {
        selector: String,
        tx: SyncSender<Result<Option<WebElementId>, TesterError>>,
    },
    QueryAll {
        selector: String,
        tx: SyncSender<Result<Vec<WebElementId>, TesterError>>,
    },
    Click {
        id: WebElementId,
        tx: SyncSender<Result<(), TesterError>>,
    },
    Html {
        id: WebElementId,
        outer: bool,
        tx: SyncSender<Result<String, TesterError>>,
    },
    Rect {
        id: WebElementId,
        tx: SyncSender<Result<ElementRect, TesterError>>,
    },
}

async fn run_desktop_commands(mut command_rx: tokio_mpsc::UnboundedReceiver<WebCommand>) {
    let mut state = DesktopWebState::new();

    while let Some(command) = command_rx.recv().await {
        match command {
            WebCommand::Build(tx) => {
                state.wait_for_render().await;
                let _ = tx.send(Ok(()));
            }
            WebCommand::Pump(tx) => {
                state.wait_for_render().await;
                let _ = tx.send(Ok(()));
            }
            WebCommand::ParseSelector { selector, tx } => {
                let _ = tx.send(state.parse_selector(&selector));
            }
            WebCommand::QueryOne { selector, tx } => {
                let result = state.query_one(&selector);
                let _ = tx.send(result);
            }
            WebCommand::QueryAll { selector, tx } => {
                let result = state.query_all(&selector);
                let _ = tx.send(result);
            }
            WebCommand::Click { id, tx } => {
                let result = state.click(id);
                let _ = tx.send(result);
            }
            WebCommand::Html { id, outer, tx } => {
                let result = state.html(id, outer);
                let _ = tx.send(result);
            }
            WebCommand::Rect { id, tx } => {
                let result = state.rect(id);
                let _ = tx.send(result);
            }
        }
    }
}

struct DesktopWebState {
    root: web_sys_x::Element,
    elements: Vec<web_sys_x::Element>,
}

impl DesktopWebState {
    fn new() -> Self {
        let root = Self::root_element().expect("missing desktop webview root element");
        Self {
            root: root.clone(),
            elements: vec![root],
        }
    }

    async fn wait_for_render(&self) {
        tokio::time::sleep(Duration::from_millis(16)).await;
    }

    fn root_element() -> Result<web_sys_x::Element, TesterError> {
        web_sys_x::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(ROOT_ID))
            .ok_or_else(|| TesterError::BackendError("missing desktop webview root element".into()))
    }

    fn parse_selector(&self, selector: &str) -> Result<(), TesterError> {
        if selector == ":root" {
            return Ok(());
        }
        self.root.query_selector(selector).map(|_| ()).map_err(|_| {
            TesterError::InvalidCssSelector(format!("Invalid CSS selector '{selector}'"))
        })
    }

    fn query_one(&mut self, selector: &str) -> Result<Option<WebElementId>, TesterError> {
        if selector == ":root" {
            return Ok(Some(WebElementId(0)));
        }
        let element = self.root.query_selector(selector).map_err(|_| {
            TesterError::InvalidCssSelector(format!("Invalid CSS selector '{selector}'"))
        })?;
        Ok(element.map(|element| self.retain_element(element)))
    }

    fn query_all(&mut self, selector: &str) -> Result<Vec<WebElementId>, TesterError> {
        if selector == ":root" {
            return Ok(vec![WebElementId(0)]);
        }

        let elements = self.root.query_selector_all(selector).map_err(|_| {
            TesterError::InvalidCssSelector(format!("Invalid CSS selector '{selector}'"))
        })?;

        let mut ids = Vec::new();
        for index in 0..elements.length() {
            if let Some(node) = elements.item(index)
                && let Ok(element) = node.dyn_into::<web_sys_x::Element>()
            {
                ids.push(self.retain_element(element));
            }
        }
        Ok(ids)
    }

    fn retain_element(&mut self, element: web_sys_x::Element) -> WebElementId {
        let id = self.elements.len();
        self.elements.push(element);
        WebElementId(id)
    }

    fn element(&self, id: WebElementId) -> Result<web_sys_x::Element, TesterError> {
        self.elements
            .get(id.0)
            .cloned()
            .ok_or_else(|| TesterError::BackendError(format!("unknown web element id {}", id.0)))
    }

    fn click(&self, id: WebElementId) -> Result<(), TesterError> {
        dispatch_click(&self.element(id)?);
        Ok(())
    }

    fn html(&self, id: WebElementId, outer: bool) -> Result<String, TesterError> {
        let element = self.element(id)?;
        Ok(if outer {
            element.outer_html()
        } else {
            element.inner_html()
        })
    }

    fn rect(&self, id: WebElementId) -> Result<ElementRect, TesterError> {
        let rect = self.element(id)?.get_bounding_client_rect();
        Ok(ElementRect {
            x: rect.x(),
            y: rect.y(),
            width: rect.width(),
            height: rect.height(),
        })
    }
}

#[wasm_bindgen(crate = wry_bindgen, inline_js = r#"
export function dispatch_click(element) {
    element.dispatchEvent(new MouseEvent("click", {
        view: window,
        bubbles: true,
        cancelable: true,
        button: 0,
    }));
}
"#)]
extern "C" {
    fn dispatch_click(element: &web_sys_x::Element);
}
