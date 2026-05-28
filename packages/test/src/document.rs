use crate::{
    condition::{AllElementsCondition, ElementCondition},
    driver::{BlitzDriver, Driver, RootContext},
    result::TesterError,
};
use dioxus_core::{Element, VirtualDom};
use std::time::Duration;

/// Returns a new Blitz-backed [DocumentTester] resulting from rendering the given [Element].
pub fn render(element: fn() -> Element) -> DocumentTester {
    DocumentTester::from_element(element)
}

/// Returns a new [DocumentTester] with the requested backend driver.
pub fn render_with_driver<D: Driver>(element: fn() -> Element) -> DocumentTester<D> {
    DocumentTester::<D>::from_element(element)
}

/// A wrapper which allows querying and interacting with a DOM in Dioxus tests.
pub struct DocumentTester<D: Driver = BlitzDriver> {
    driver: D,
}

impl<D: Driver> DocumentTester<D> {
    /// Constructs a new instance by rendering the given `element`.
    pub fn from_element(element: fn() -> Element) -> Self {
        Self {
            driver: D::from_element(element),
        }
    }

    /// Adds the given context to the root of this tester's virtual DOM.
    ///
    /// The context is available to all elements within the DOM.
    ///
    /// See Dioxus documentation for more information on context.
    pub fn with_root_context<T: Clone + 'static>(mut self, context: T) -> Self
    where
        D: RootContext<T>,
    {
        self.driver.with_root_context(context);
        self
    }

    /// Sets the size of the window in pixels to which this DOM will virtually render.
    pub fn with_window_size(mut self, width: u32, height: u32) -> Self {
        self.driver.with_window_size(width, height);
        self
    }

    /// Performs a layout and build for the DOM managed by this tester.
    ///
    /// This method must be invoked before querying any elements.
    pub fn build(mut self) -> Self {
        self.driver.build();
        self
    }

    /// Resolve a single round of asynchronous operations via the async runtime and the Dioxus
    /// runtime.
    ///
    /// This waits for one pending unit of Dioxus work, flushes it through the active backend, and
    /// returns [TesterError::PumpTimeout] if no work appears before the backend timeout elapses.
    pub async fn pump(&mut self) -> Result<(), TesterError> {
        self.driver.pump().await
    }

    /// Advance the internal clock by the given [Duration].
    ///
    /// The Blitz backend uses this to resolve layout and animations. Backends that read from a live
    /// renderer may ignore synthetic time advancement.
    pub async fn advance_time(&mut self, duration: Duration) {
        self.driver.advance_time(duration).await;
    }

    /// Returns an element referencing the root DOM node managed by this tester.
    ///
    /// This allows interacting with and asserting on the root element. To await expectations on the
    /// root element, use [Self::query] with the CSS selector `:root`.
    pub fn root<'vdom>(&'vdom self) -> D::Element<'vdom> {
        self.driver.root()
    }

    pub(crate) fn get_element(&self, query: &D::Selector) -> Option<D::ElementId> {
        self.driver.get_element(query)
    }

    pub(crate) fn get_elements(&self, query: &D::Selector) -> Vec<D::ElementId> {
        self.driver.get_elements(query)
    }

    /// Returns a representation of first element in the DOM satisfying the given query.
    ///
    /// The query can be any value accepted by [TryIntoSelector], including CSS selector strings and
    /// [by_testid] queries. See [ElementCondition] for the awaitable operations available on the
    /// returned value.
    ///
    /// Panics if the query contains a syntactically invalid CSS selector.
    pub fn query(&mut self, query: impl TryIntoSelector) -> ElementCondition<'_, D> {
        let query = query.into_selector_query();
        let selector = self
            .driver
            .parse_selector(&query.selector)
            .expect("Invalid CSS selector");
        ElementCondition::new(self, selector, query.error)
    }

    /// Returns a representation of elements in the DOM satisfying the given query.
    ///
    /// The query can be any value accepted by [TryIntoSelector], including CSS selector strings and
    /// [by_testid] queries. See [AllElementsCondition] for the awaitable operations available on
    /// the returned value.
    ///
    /// Panics if the query contains a syntactically invalid CSS selector.
    pub fn query_all(&mut self, query: impl TryIntoSelector) -> AllElementsCondition<'_, D> {
        let query = query.into_selector_query();
        let selector = self
            .driver
            .parse_selector(&query.selector)
            .expect("Invalid CSS selector");
        AllElementsCondition::new(self, selector)
    }

    pub(crate) fn build_resolved_element(&self, id: D::ElementId) -> D::Element<'_> {
        self.driver.build_element(id)
    }
}

impl DocumentTester<BlitzDriver> {
    /// Constructs a new Blitz-backed instance from the given [VirtualDom].
    pub fn from_virtual_dom(virtual_dom: VirtualDom) -> Self {
        Self {
            driver: BlitzDriver::from_virtual_dom(virtual_dom),
        }
    }
}

/// A backend-independent CSS selector query.
pub struct SelectorQuery {
    selector: String,
    error: TesterError,
}

impl SelectorQuery {
    /// Create a selector query with the error to report if no element matches.
    pub fn new(selector: impl Into<String>, error: TesterError) -> Self {
        Self {
            selector: selector.into(),
            error,
        }
    }
}

/// A value which can be turned into a CSS selector to query the DOM.
///
/// This is implemented for all types which dereference to `str`, including `&str` and `String`.
///
/// One can also select by [testid](https://testing-library.com/docs/queries/bytestid/) using the
/// function [by_testid].
pub trait TryIntoSelector {
    /// Convert this value into a backend-independent selector query.
    fn into_selector_query(self) -> SelectorQuery;
}

impl<T: AsRef<str>> TryIntoSelector for T {
    fn into_selector_query(self) -> SelectorQuery {
        SelectorQuery::new(
            self.as_ref(),
            TesterError::NoSuchElementWithCssSelector(self.as_ref().into()),
        )
    }
}

struct QueryByTestId(String);

impl TryIntoSelector for QueryByTestId {
    fn into_selector_query(self) -> SelectorQuery {
        let testid = self.0;
        SelectorQuery::new(
            format!(r#"[data-testid="{testid}"]"#),
            TesterError::NoSuchElementWithTestId(testid),
        )
    }
}

/// Returns a query selector matching elements with the given value in the `data-testid` attribute.
///
/// This attribute is a common convention for marking DOM components with which tests interact.
pub fn by_testid(testid: impl AsRef<str>) -> impl TryIntoSelector {
    QueryByTestId(testid.as_ref().to_string())
}
