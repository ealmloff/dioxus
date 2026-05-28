use crate::{
    element::{NodeId, ResolvedElement},
    result::TesterError,
};
use blitz_dom::{Document as _, SelectorList};
use dioxus_core::{Element, VirtualDom};
use dioxus_html::geometry::Coordinates;
use dioxus_native_dom::{DioxusDocument, DocumentConfig};
use std::{fmt::Debug, future::Future, time::Duration};
use tokio::time::timeout;

/// The maximum time a driver will wait for new Dioxus work before concluding
/// that no new work is forthcoming.
pub(crate) const PUMP_TIMEOUT: Duration = Duration::from_millis(1000);

/// An element handle returned by a [`Driver`].
pub trait TestElement: Debug {
    /// Dispatch a click event on this element.
    fn click(&self);

    /// Return this element and its descendants as HTML.
    fn outer_html(&self) -> String;

    /// Return this element's descendants as HTML.
    fn inner_html(&self) -> String;

    /// Return the calculated center point of this element.
    fn center(&self) -> Coordinates;

    /// Return the calculated upper-left point of this element.
    fn upper_left(&self) -> Coordinates;

    /// Return the calculated upper-right point of this element.
    fn upper_right(&self) -> Coordinates;

    /// Return the calculated lower-left point of this element.
    fn lower_left(&self) -> Coordinates;

    /// Return the calculated lower-right point of this element.
    fn lower_right(&self) -> Coordinates;

    /// Return the calculated size of this element.
    fn size(&self) -> (f32, f32);
}

/// A backend that can render, query, read, and interact with a Dioxus DOM for tests.
pub trait Driver: Sized {
    /// Parsed selector representation used by this driver.
    type Selector;

    /// Stable element identifier used between query and element resolution.
    type ElementId: Copy + Debug;

    /// Element handle returned by this driver.
    type Element<'driver>: TestElement
    where
        Self: 'driver;

    /// Construct a driver from a root component.
    fn from_element(element: fn() -> Element) -> Self;

    /// Configure the driver viewport size.
    fn with_window_size(&mut self, width: u32, height: u32);

    /// Perform the initial build.
    fn build(&mut self);

    /// Drive one round of pending work.
    fn pump(&mut self) -> impl Future<Output = Result<(), TesterError>>;

    /// Advance driver time.
    fn advance_time(&mut self, duration: Duration) -> impl Future<Output = ()>;

    /// Parse a CSS selector.
    fn parse_selector(&self, selector: &str) -> Result<Self::Selector, TesterError>;

    /// Resolve the root element.
    fn root(&self) -> Self::Element<'_>;

    /// Resolve the first matching element id.
    fn get_element(&self, selector: &Self::Selector) -> Option<Self::ElementId>;

    /// Resolve all matching element ids.
    fn get_elements(&self, selector: &Self::Selector) -> Vec<Self::ElementId>;

    /// Build an element handle from an element id.
    fn build_element(&self, id: Self::ElementId) -> Self::Element<'_>;
}

/// A driver backend that supports adding root context values.
pub trait RootContext<T: Clone + 'static> {
    /// Add context to the root of the driver-managed virtual DOM.
    fn with_root_context(&mut self, context: T);
}

/// The default headless driver backed by `dioxus-native-dom` and Blitz.
pub struct BlitzDriver {
    document: DioxusDocument,
    now: f64,
    window_size: Option<(u32, u32)>,
}

impl BlitzDriver {
    /// Constructs a new instance from the given [VirtualDom].
    pub fn from_virtual_dom(virtual_dom: VirtualDom) -> Self {
        let document = DioxusDocument::new(virtual_dom, DocumentConfig::default());
        Self {
            document,
            now: 0.0,
            window_size: None,
        }
    }
}

impl Driver for BlitzDriver {
    type Selector = SelectorList;
    type ElementId = NodeId;
    type Element<'driver> = ResolvedElement<'driver>;

    fn from_element(element: fn() -> Element) -> Self {
        Self::from_virtual_dom(VirtualDom::new(element))
    }

    fn with_window_size(&mut self, width: u32, height: u32) {
        self.window_size = Some((width, height));
    }

    fn build(&mut self) {
        self.document.inner_mut().viewport_mut().window_size =
            self.window_size.unwrap_or((500, 800));
        self.document.initial_build();
        self.document.inner_mut().resolve(self.now);
    }

    async fn pump(&mut self) -> Result<(), TesterError> {
        timeout(PUMP_TIMEOUT, self.document.vdom.wait_for_work())
            .await
            .map_err(|_| TesterError::PumpTimeout)?;
        while self.document.poll(None) {}
        Ok(())
    }

    async fn advance_time(&mut self, duration: Duration) {
        self.now += duration.as_secs_f64();
        self.document.inner_mut().resolve(self.now);
    }

    fn parse_selector(&self, selector: &str) -> Result<Self::Selector, TesterError> {
        self.document
            .inner()
            .try_parse_selector_list(selector)
            .map_err(|_| {
                TesterError::InvalidCssSelector(format!("Invalid CSS selector '{selector}'"))
            })
    }

    fn root(&self) -> Self::Element<'_> {
        let document = self.document.inner();
        ResolvedElement {
            vdom: &self.document.vdom,
            document,
            node_id: NodeId::Root,
        }
    }

    fn get_element(&self, selector: &Self::Selector) -> Option<Self::ElementId> {
        self.document
            .inner()
            .query_selector_raw(selector)
            .map(NodeId::Node)
    }

    fn get_elements(&self, selector: &Self::Selector) -> Vec<Self::ElementId> {
        self.document
            .inner()
            .query_selector_all_raw(selector)
            .iter()
            .copied()
            .map(NodeId::Node)
            .collect()
    }

    fn build_element(&self, id: Self::ElementId) -> Self::Element<'_> {
        ResolvedElement {
            vdom: &self.document.vdom,
            document: self.document.inner(),
            node_id: id,
        }
    }
}

impl<T: Clone + 'static> RootContext<T> for BlitzDriver {
    fn with_root_context(&mut self, context: T) {
        self.document.vdom.provide_root_context(context);
    }
}
