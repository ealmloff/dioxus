use crate::{
    blitz_driver::BlitzDriver,
    condition::{AllElementsCondition, ElementCondition},
    driver::Driver,
    element::ResolvedElement,
    result::TesterError,
};
use dioxus_core::{Element, VirtualDom};
use std::time::Duration;

/// Returns a new [DocumentTester] resulting from rendering the given [Element].
///
/// Backed by [BlitzDriver].
pub fn render(element: fn() -> Element) -> DocumentTester {
    DocumentTester::from_element(element)
}

/// A wrapper which allows querying and interacting with a DOM in Dioxus tests.
///
/// Generic over a [Driver] backend. The default, [BlitzDriver], runs the DOM in-process. To
/// drive a remote renderer (such as a browser via CDP or WebDriver) implement [Driver] for that
/// transport and construct the tester via [DocumentTester::with_driver].
pub struct DocumentTester<D: Driver = BlitzDriver> {
    pub(crate) driver: D,
}

impl DocumentTester<BlitzDriver> {
    /// Constructs a new instance by rendering the given `element`.
    pub fn from_element(element: fn() -> Element) -> Self {
        Self::from_virtual_dom(VirtualDom::new(element))
    }

    /// Constructs a new instance from the given [VirtualDom].
    pub fn from_virtual_dom(virtual_dom: VirtualDom) -> Self {
        Self {
            driver: BlitzDriver::from_virtual_dom(virtual_dom),
        }
    }

    /// Adds the given context to the root of this tester's virtual DOM.
    ///
    /// The context is available to all elements within the DOM. See the
    /// [Dioxus documentation](https://dioxuslabs.com/learn/0.7/essentials/basics/context) for
    /// more information on context.
    pub fn with_root_context<T: Clone + 'static>(self, context: T) -> Self {
        self.driver.provide_root_context(context);
        self
    }

    /// Sets the size of the window in pixels to which this DOM will virtually render.
    pub fn with_window_size(mut self, width: u32, height: u32) -> Self {
        self.driver.set_window_size(width, height);
        self
    }

    /// Sets the maximum time [Self::pump] will wait for new events before concluding that no
    /// further events are forthcoming.
    pub fn with_pump_timeout(mut self, timeout: Duration) -> Self {
        self.driver.set_pump_timeout(timeout);
        self
    }

    /// Sets the maximum number of pump iterations the tester will perform while waiting for an
    /// element or for an assertion to hold.
    pub fn with_max_tries(mut self, max_tries: usize) -> Self {
        self.driver.set_max_tries(max_tries);
        self
    }

    /// Performs a layout and build for the DOM managed by this tester.
    ///
    /// Must be invoked before querying any elements.
    pub fn build(mut self) -> Self {
        self.driver.build();
        self
    }

    /// Advance the internal clock by the given [Duration].
    ///
    /// This advances any CSS animations which may be in progress and recalculates the layout.
    pub fn advance_time(&mut self, duration: Duration) {
        self.driver.advance_time(duration);
    }
}

impl<D: Driver> DocumentTester<D> {
    /// Constructs a tester wrapping a custom [Driver].
    ///
    /// Backend-specific configuration (window size, pump timeout, time advancement, etc.) is not
    /// available through this generic surface — the `with_*` builders on
    /// [`DocumentTester<BlitzDriver>`] are Blitz-only. Reach the underlying driver via
    /// [`Self::driver_mut`] to call backend-specific methods.
    pub fn with_driver(driver: D) -> Self {
        Self { driver }
    }

    /// Returns a reference to the underlying driver.
    pub fn driver(&self) -> &D {
        &self.driver
    }

    /// Returns a mutable reference to the underlying driver.
    pub fn driver_mut(&mut self) -> &mut D {
        &mut self.driver
    }

    /// Resolve one round of asynchronous work via the driver.
    ///
    /// Each call resolves either a Dioxus event handler invocation or one round of async work
    /// external to Dioxus (such as a network request). Use multiple calls to step through the
    /// stages of a workflow.
    ///
    /// Returns [`Err(PumpTimeout)`](crate::PumpTimeout) if the driver gave up waiting for new
    /// work — informational only. Tests that don't care can discard the result.
    ///
    /// ```no_run
    /// # use dioxus::prelude::*;
    /// # #[component]
    /// # fn AComponent() -> Element { rsx! { } }
    /// # async fn run_test() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut tester = dioxus_test::render(AComponent).build();
    /// tester.query(".make-request-button").click().await?;
    /// let _ = tester.pump().await; // React to the click
    /// // Assert on UI state while the request is in flight.
    ///
    /// let _ = tester.pump().await; // Receive the response
    /// // Assert on UI state after the response.
    /// # Ok(())
    /// # }
    /// ```
    pub async fn pump(&mut self) -> Result<(), crate::PumpTimeout> {
        self.driver.pump().await
    }

    /// Returns an element referencing the root DOM node managed by this tester.
    ///
    /// `async` so remote backends can fetch the root handle over a transport; the in-process
    /// [BlitzDriver] resolves immediately. There is no support for *awaiting* an expectation on
    /// the root — to do that, drive the event loop with `tester.pump().await` between
    /// assertions.
    pub async fn root(&self) -> ResolvedElement<'_, D> {
        let handle = self.driver.root().await;
        ResolvedElement {
            handle,
            driver: &self.driver,
        }
    }

    /// Returns a representation of the first element in the DOM satisfying the given query.
    ///
    /// The query can be anything which dereferences to a `str`, or the result of [by_testid].
    ///
    /// If the query contains a syntactically invalid CSS selector, the returned
    /// [ElementCondition] yields [TesterError::InvalidCssSelector] when awaited.
    ///
    /// Awaiting a query for a selector that will never match drives the event loop up to
    /// [Driver::max_tries] times before returning [TesterError::NoSuchElementWithCssSelector]
    /// or [TesterError::NoSuchElementWithTestId]. With the [BlitzDriver] defaults
    /// ([DEFAULT_MAX_TRIES] iterations × [DEFAULT_PUMP_TIMEOUT] per pump) that's a worst-case
    /// wait on the order of a few seconds; tune via [DocumentTester::with_max_tries] and
    /// [DocumentTester::with_pump_timeout] if needed.
    ///
    /// ```rust
    /// # use dioxus::prelude::*;
    /// # use dioxus_test::*;
    /// #[component]
    /// fn AComponent() -> Element {
    ///    let mut click_count = use_signal(|| 0);
    ///    rsx! {
    ///        button { onclick: move |_| click_count += 1, "Click me!" }
    ///        div { id: "click-count", "Click count: {click_count}" }
    ///    }
    /// }
    /// # async fn run_test() -> std::result::Result<(), Box<dyn std::error::Error>> {
    /// let mut tester = dioxus_test::render(AComponent).build();
    /// tester.query("#click-count").expect(inner_html(contains_string("Click count: 0"))).await?;
    /// tester.query("button").click().await?;
    /// tester.query("#click-count").expect(inner_html(contains_string("Click count: 1"))).await?;
    /// # Ok(())
    /// # }
    /// # tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(run_test()).unwrap();
    /// ```
    pub fn query<Q>(&mut self, query: Q) -> ElementCondition<'_, D>
    where
        Q: TryIntoSelector<D>,
    {
        let not_found_error = query.to_tester_error();
        let selector = query.try_into_selector(&self.driver);
        ElementCondition::new(self, selector, not_found_error)
    }

    /// Returns a representation of all elements in the DOM satisfying the given query.
    pub fn query_all<Q>(&mut self, query: Q) -> AllElementsCondition<'_, D>
    where
        Q: TryIntoSelector<D>,
    {
        let selector = query.try_into_selector(&self.driver);
        AllElementsCondition::new(self, selector)
    }
}

/// A value which can be turned into a selector parsed by a [Driver].
///
/// Implemented for all types which dereference to `str` (e.g. `&str`, `String`), interpreted as
/// CSS selectors. Also implemented by the return value of [by_testid].
pub trait TryIntoSelector<D: Driver> {
    fn try_into_selector(self, driver: &D) -> Result<D::Selector, TesterError>;
    fn to_tester_error(&self) -> TesterError;
}

impl<T: AsRef<str>, D: Driver> TryIntoSelector<D> for T {
    fn try_into_selector(self, driver: &D) -> Result<D::Selector, TesterError> {
        driver.parse_selector(self.as_ref())
    }

    fn to_tester_error(&self) -> TesterError {
        TesterError::NoSuchElementWithCssSelector(self.as_ref().into())
    }
}

/// Returned by [by_testid]. Wraps a test-id value and parses to the corresponding
/// `[data-testid="..."]` CSS selector.
///
/// Construct via [by_testid] — the inner value is intentionally private so the only way to
/// produce a `QueryByTestId` is through the public constructor.
pub struct QueryByTestId(pub(crate) String);

impl<D: Driver> TryIntoSelector<D> for QueryByTestId {
    fn try_into_selector(self, driver: &D) -> Result<D::Selector, TesterError> {
        driver.parse_selector(&format!(r#"[data-testid="{}"]"#, self.0))
    }

    fn to_tester_error(&self) -> TesterError {
        TesterError::NoSuchElementWithTestId(self.0.clone())
    }
}

/// Returns a selector matching elements with the given value in the `data-testid` attribute.
///
/// ```
/// use dioxus::prelude::*;
/// use dioxus_test::{by_testid, eq, inner_html, render};
///
/// #[component]
/// fn MyComponent() -> Element {
///     rsx! { div { "data-testid": "the-label", "Label content" } }
/// }
///
/// # /* Make sure this also compiles as a doctest.
/// #[tokio::test]
/// # */
/// async fn label_renders() {
///     let mut tester = render(MyComponent).build();
///     tester
///         .query(by_testid("the-label"))
///         .expect(inner_html(eq("Label content")))
///         .await
///         .unwrap();
/// }
/// # tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(label_renders());
/// ```
pub fn by_testid(testid: impl AsRef<str>) -> QueryByTestId {
    QueryByTestId(testid.as_ref().to_string())
}
