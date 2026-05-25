use crate::{
    TesterError,
    driver::{BoundingBox, Driver, PumpTimeout},
};
use blitz_dom::{DocGuard, Document as _, Node, SelectorList};
use dioxus_core::{ElementId, Event, VirtualDom};
use dioxus_html::{Modifiers, PlatformEventData};
use dioxus_native_dom::{DioxusDocument, DocumentConfig, synthetic_click_event};
use std::{any::Any, rc::Rc, time::Duration};
use tokio::time::timeout;

/// Default value for [BlitzDriver::set_pump_timeout].
pub const DEFAULT_PUMP_TIMEOUT: Duration = Duration::from_millis(1000);

/// Default value for [BlitzDriver::set_max_tries].
pub const DEFAULT_MAX_TRIES: usize = 5;

/// An opaque handle into the DOM owned by a [BlitzDriver].
///
/// Constructed only by the driver itself: [Driver::root] returns a root handle, and
/// [Driver::query] / [Driver::query_all] return per-node handles. Holding a handle is just a
/// token — feeding one back through the driver routes to an actual DOM node.
#[derive(Debug, Clone, Copy)]
pub struct BlitzNodeHandle(BlitzNodeHandleKind);

#[derive(Debug, Clone, Copy)]
enum BlitzNodeHandleKind {
    Root,
    Node(usize),
}

impl BlitzNodeHandle {
    pub(crate) fn root() -> Self {
        Self(BlitzNodeHandleKind::Root)
    }

    pub(crate) fn node(id: usize) -> Self {
        Self(BlitzNodeHandleKind::Node(id))
    }
}

/// In-process [Driver] backed by [DioxusDocument] and Blitz layout.
///
/// Drives the Dioxus runtime and Blitz layout in the test process. Constructed indirectly through
/// [crate::render] / [crate::DocumentTester::from_element]. Backend-specific operations such as
/// [Self::advance_time] are reachable via [crate::DocumentTester::driver_mut].
pub struct BlitzDriver {
    document: DioxusDocument,
    now: f64,
    window_size: Option<(u32, u32)>,
    pump_timeout: Duration,
    max_tries: usize,
}

impl BlitzDriver {
    /// Constructs a [BlitzDriver] wrapping the given [VirtualDom].
    pub fn from_virtual_dom(virtual_dom: VirtualDom) -> Self {
        let document = DioxusDocument::new(virtual_dom, DocumentConfig::default());
        Self::with_document(document)
    }

    fn with_document(document: DioxusDocument) -> Self {
        Self {
            document,
            now: 0.0,
            window_size: None,
            pump_timeout: DEFAULT_PUMP_TIMEOUT,
            max_tries: DEFAULT_MAX_TRIES,
        }
    }

    /// Sets the virtual viewport size in pixels.
    pub fn set_window_size(&mut self, width: u32, height: u32) {
        self.window_size = Some((width, height));
    }

    /// Sets the maximum time [Driver::pump] will wait for new events.
    pub fn set_pump_timeout(&mut self, timeout: Duration) {
        self.pump_timeout = timeout;
    }

    /// Sets the maximum number of pump iterations performed while waiting for an element or
    /// assertion.
    pub fn set_max_tries(&mut self, max_tries: usize) {
        self.max_tries = max_tries;
    }

    /// Provides a context value to the root of the virtual DOM.
    ///
    /// Takes `&self` because [`VirtualDom`] handles its own interior synchronization for context
    /// mutation; no exclusive borrow is required at this layer.
    pub fn provide_root_context<T: Clone + 'static>(&self, context: T) {
        self.document.vdom.provide_root_context(context);
    }

    /// Performs the initial layout and build.
    pub fn build(&mut self) {
        self.document.inner_mut().viewport_mut().window_size =
            self.window_size.unwrap_or((500, 800));
        self.document.initial_build();
        self.document.inner_mut().resolve(self.now);
    }

    /// Advances the internal clock by the given duration and re-resolves layout.
    pub fn advance_time(&mut self, duration: Duration) {
        self.now += duration.as_secs_f64();
        self.document.inner_mut().resolve(self.now);
    }

    /// Dispatches an event with the given `name` to the node identified by `handle`.
    ///
    /// This is the Blitz-specific escape hatch for events other than `click`. Remote backends
    /// (CDP/WebDriver) model input differently, so this is intentionally not part of the
    /// [Driver] trait. Reach it via `tester.driver_mut().send_event(handle, ...)`.
    ///
    /// The event is registered with the Dioxus runtime. A subsequent [`Driver::pump`] causes the
    /// event handler to be invoked, if one is present. If the node has no associated Dioxus
    /// element id, this call is a no-op.
    ///
    /// The `event` parameter must wrap a [`PlatformEventData`] whose payload corresponds to the
    /// dispatched event type. A mismatch is *not* caught here; the panic happens later, inside
    /// the event handler invocation during a subsequent [`Driver::pump`].
    pub fn send_event(
        &self,
        handle: BlitzNodeHandle,
        name: &str,
        event: Event<PlatformEventData>,
    ) {
        let element_id = {
            let doc = self.document.inner();
            get_dioxus_element_id(resolve_node(handle, &doc))
        };
        let Some(element_id) = element_id else { return };
        let propagates = event.propagates();
        self.document
            .vdom
            .runtime()
            .handle_event(name, Event::new(event.data, propagates), element_id);
    }

    /// Direct access to the underlying [DioxusDocument]. Use with care; bypassing the
    /// [Driver] trait sidesteps the abstraction other drivers must implement.
    pub fn document(&self) -> &DioxusDocument {
        &self.document
    }

    /// Mutable access to the underlying [DioxusDocument].
    pub fn document_mut(&mut self) -> &mut DioxusDocument {
        &mut self.document
    }

    /// Borrows the [`Node`] referenced by `handle` for the duration of `f`.
    ///
    /// The companion to [`Self::send_event`]: synthetic event constructors such as
    /// [`dioxus_native_dom::synthetic_click_event`] need a `&Node`, but [BlitzNodeHandle] is
    /// opaque. This method bridges the two without leaking the underlying node id.
    pub fn with_node<R>(&self, handle: BlitzNodeHandle, f: impl FnOnce(&Node) -> R) -> R {
        let doc = self.document.inner();
        f(resolve_node(handle, &doc))
    }
}

impl Driver for BlitzDriver {
    type NodeHandle = BlitzNodeHandle;
    type Selector = SelectorList;

    fn parse_selector(&self, selector: &str) -> Result<Self::Selector, TesterError> {
        self.document
            .inner()
            .try_parse_selector_list(selector)
            .map_err(|_| {
                TesterError::InvalidCssSelector(format!("Invalid CSS selector '{selector}'"))
            })
    }

    fn max_tries(&self) -> usize {
        self.max_tries
    }

    async fn root(&self) -> BlitzNodeHandle {
        BlitzNodeHandle::root()
    }

    async fn query(&self, selector: &SelectorList) -> Option<BlitzNodeHandle> {
        self.document
            .inner()
            .query_selector_raw(selector)
            .map(BlitzNodeHandle::node)
    }

    async fn query_all(&self, selector: &SelectorList) -> Vec<BlitzNodeHandle> {
        self.document
            .inner()
            .query_selector_all_raw(selector)
            .iter()
            .map(|id| BlitzNodeHandle::node(*id))
            .collect()
    }

    async fn inner_html(&self, handle: BlitzNodeHandle) -> String {
        let doc = self.document.inner();
        let node = resolve_node(handle, &doc);
        let parts: Vec<_> = node
            .children
            .iter()
            .filter_map(|child_id| doc.get_node(*child_id).map(|c| c.outer_html()))
            .collect();
        parts.join("")
    }

    async fn outer_html(&self, handle: BlitzNodeHandle) -> String {
        let doc = self.document.inner();
        resolve_node(handle, &doc).outer_html()
    }

    async fn bounding_box(&self, handle: BlitzNodeHandle) -> BoundingBox {
        let doc = self.document.inner();
        let node = resolve_node(handle, &doc);
        BoundingBox {
            x: node.final_layout.location.x as f64,
            y: node.final_layout.location.y as f64,
            width: node.final_layout.content_box_width() as f64,
            height: node.final_layout.content_box_height() as f64,
        }
    }

    async fn click(&self, handle: BlitzNodeHandle) {
        let (element_id, event_data) = {
            let doc = self.document.inner();
            let node = resolve_node(handle, &doc);
            let element_id = get_dioxus_element_id(node);
            let event_data = synthetic_click_event(node, Modifiers::empty());
            (element_id, event_data)
        };
        let Some(element_id) = element_id else { return };
        let data: Rc<dyn Any> = Rc::new(PlatformEventData::new(event_data));
        self.document
            .vdom
            .runtime()
            .handle_event("click", Event::new(data, true), element_id);
    }

    async fn pump(&mut self) -> Result<(), PumpTimeout> {
        let wait_outcome = timeout(self.pump_timeout, self.document.vdom.wait_for_work()).await;
        while self.document.poll(None) {}
        match wait_outcome {
            Ok(()) => Ok(()),
            Err(_) => Err(PumpTimeout),
        }
    }
}

fn resolve_node<'a>(handle: BlitzNodeHandle, doc: &'a DocGuard<'a>) -> &'a Node {
    match handle.0 {
        BlitzNodeHandleKind::Root => doc.root_element(),
        BlitzNodeHandleKind::Node(node_id) => doc
            .get_node(node_id)
            .expect("Element must be attached"),
    }
}

fn get_dioxus_element_id(node: &Node) -> Option<ElementId> {
    node.element_data()?
        .attrs
        .iter()
        .find(|attr| *attr.name.local == *"data-dioxus-id")
        .and_then(|attr| attr.value.parse::<usize>().ok())
        .map(ElementId)
}
