use crate::{
    DocumentTester, Matcher, TesterError,
    driver::Driver,
    element::ResolvedElement,
};
use std::{ops::ControlFlow, pin::Pin};

/// A representation of a single element on the DOM which may already exist or may exist in the
/// future.
///
/// A test can make assertions on the element with [ElementCondition::expect]. The test decides
/// whether to make the assertion immediately or await it.
///
/// ```
/// use dioxus::prelude::*;
/// use dioxus_test::{eq, inner_html, render};
///
/// #[component]
/// fn MyComponent() -> Element {
///     rsx! { div { class: "test-component", "Hello, world!" } }
/// }
///
/// # /* Make sure this also compiles as a doctest.
/// #[tokio::test]
/// # */
/// async fn my_component_renders_correctly() {
///     let mut tester = render(MyComponent).build();
///     tester.query(".test-component").expect(inner_html(eq("Hello, world!"))).await.unwrap();
/// }
/// # tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(my_component_renders_correctly());
/// ```
pub struct ElementCondition<'vdom, D: Driver> {
    tester: &'vdom mut DocumentTester<D>,
    selector: Result<D::Selector, TesterError>,
    not_found_error: TesterError,
}

impl<'vdom, D: Driver> ElementCondition<'vdom, D> {
    pub(crate) fn new(
        tester: &'vdom mut DocumentTester<D>,
        selector: Result<D::Selector, TesterError>,
        not_found_error: TesterError,
    ) -> Self {
        Self {
            tester,
            selector,
            not_found_error,
        }
    }

    /// Simulates the user clicking on the element this instance represents.
    ///
    /// Drives the event loop until the element appears, up to [Driver::max_tries] iterations.
    /// Returns `Err` if the element does not appear.
    pub async fn click(self) -> Result<(), TesterError> {
        let element = self.into_future().await?;
        element.click().await;
        Ok(())
    }

    /// Synonym for [ElementCondition::click].
    pub fn tap(self) -> impl Future<Output = Result<(), TesterError>> + 'vdom {
        self.click()
    }

    /// Asserts that the given [Matcher] matches this element, either immediately or in the future.
    ///
    /// ```
    /// use dioxus::prelude::*;
    /// use dioxus_test::{eq, inner_html, render};
    ///
    /// #[component]
    /// fn MyComponent() -> Element {
    ///     rsx! { div { class: "test-component", "Hello, world!" } }
    /// }
    ///
    /// # /* Make sure this also compiles as a doctest.
    /// #[tokio::test]
    /// # */
    /// async fn my_component_renders_correctly() {
    ///     let mut tester = render(MyComponent).build();
    ///     tester
    ///         .query(".test-component")
    ///         .expect(inner_html(eq("Hello, world!")))
    ///         .await
    ///         .unwrap();
    /// }
    /// # tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(my_component_renders_correctly());
    /// ```
    ///
    /// > Warning! Awaiting an expectation passes _as soon as the expectation is true_. This can
    /// > lead to spurious passes — first await an effect that implies the event handler ran, then
    /// > assert on state.
    pub fn expect<M>(self, matcher: M) -> ElementMatcherCondition<'vdom, D, M>
    where
        M: for<'a> Matcher<ResolvedElement<'a, D>>,
    {
        ElementMatcherCondition {
            tester: self.tester,
            selector: self.selector,
            not_found_error: self.not_found_error,
            matcher,
        }
    }

    /// Resolves the element immediately, without driving the event loop.
    ///
    /// Returns the not-found / parse error if the element is not currently present.
    pub async fn immediately(self) -> Result<ResolvedElement<'vdom, D>, TesterError> {
        let selector = self.selector?;
        let driver = &self.tester.driver;
        match driver.query(&selector).await {
            None => Err(self.not_found_error),
            Some(handle) => Ok(ResolvedElement { handle, driver }),
        }
    }
}

impl<'vdom, D: Driver> IntoFuture for ElementCondition<'vdom, D> {
    type Output = Result<ResolvedElement<'vdom, D>, TesterError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + 'vdom>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let Self {
                tester,
                selector,
                not_found_error,
            } = self;
            let selector = selector?;
            let handle = wait_for_handle(tester, &selector, not_found_error).await?;
            Ok(ResolvedElement {
                handle,
                driver: &tester.driver,
            })
        })
    }
}

/// Repeatedly query the driver for the first node matching `selector`, pumping the event loop
/// up to `max_tries()` iterations between attempts.
async fn wait_for_handle<D: Driver>(
    tester: &mut DocumentTester<D>,
    selector: &D::Selector,
    not_found_error: TesterError,
) -> Result<D::NodeHandle, TesterError> {
    let max_tries = tester.driver.max_tries();
    let mut tries = 0;
    loop {
        if let Some(handle) = tester.driver.query(selector).await {
            return Ok(handle);
        }
        tries += 1;
        if tries >= max_tries {
            return Err(not_found_error);
        }
        let _ = tester.driver.pump().await;
    }
}

/// A representation of a set of elements on the DOM matching a query.
///
/// ```
/// use dioxus::prelude::*;
/// use dioxus_test::{empty, not, render};
///
/// #[component]
/// fn MyComponent() -> Element {
///     rsx! { div { class: "test-component", "Hello, world!" } }
/// }
///
/// # /* Make sure this also compiles as a doctest.
/// #[tokio::test]
/// # */
/// async fn my_component_renders_correctly() {
///     let mut tester = render(MyComponent).build();
///     tester.query_all(".test-component").expect(not(empty())).await.unwrap();
///     tester.query_all(".missing").expect(empty()).await.unwrap();
/// }
/// # tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(my_component_renders_correctly());
/// ```
pub struct AllElementsCondition<'vdom, D: Driver> {
    tester: &'vdom mut DocumentTester<D>,
    selector: Result<D::Selector, TesterError>,
}

impl<'vdom, D: Driver> AllElementsCondition<'vdom, D> {
    pub(crate) fn new(
        tester: &'vdom mut DocumentTester<D>,
        selector: Result<D::Selector, TesterError>,
    ) -> Self {
        Self { tester, selector }
    }

    /// Asserts that the given [Matcher] matches this collection.
    pub fn expect<M>(self, matcher: M) -> AllElementsMatcherCondition<'vdom, D, M>
    where
        M: for<'a> Matcher<Vec<ResolvedElement<'a, D>>>,
    {
        AllElementsMatcherCondition {
            tester: self.tester,
            selector: self.selector,
            matcher,
        }
    }

    /// Resolves the matched elements immediately, without driving the event loop.
    pub async fn immediately(self) -> Result<Vec<ResolvedElement<'vdom, D>>, TesterError> {
        let selector = self.selector?;
        let driver = &self.tester.driver;
        Ok(driver
            .query_all(&selector)
            .await
            .into_iter()
            .map(|handle| ResolvedElement { handle, driver })
            .collect())
    }
}

/// A pending assertion of a [Matcher] against a single [ResolvedElement].
///
/// Produced by [ElementCondition::expect]. Awaiting it drives the event loop until the matcher
/// succeeds or [Driver::max_tries] iterations have passed.
pub struct ElementMatcherCondition<'vdom, D: Driver, M> {
    tester: &'vdom mut DocumentTester<D>,
    selector: Result<D::Selector, TesterError>,
    not_found_error: TesterError,
    matcher: M,
}

impl<'vdom, D: Driver, M> ElementMatcherCondition<'vdom, D, M>
where
    M: for<'a> Matcher<ResolvedElement<'a, D>>,
{
    /// Asserts that the matcher matches the element immediately, without driving the event loop.
    pub async fn immediately(self) -> Result<(), TesterError> {
        let Self {
            tester,
            selector,
            not_found_error,
            matcher,
            ..
        } = self;
        let selector = selector?;
        let driver = &tester.driver;
        let Some(handle) = driver.query(&selector).await else {
            return Err(not_found_error);
        };
        check_element(driver, handle, &matcher).await
    }
}

impl<'vdom, D: Driver, M> IntoFuture for ElementMatcherCondition<'vdom, D, M>
where
    M: for<'a> Matcher<ResolvedElement<'a, D>> + 'vdom,
{
    type Output = Result<(), TesterError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + 'vdom>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(wait_for_element_match(
            self.tester,
            self.selector,
            self.not_found_error,
            self.matcher,
        ))
    }
}

/// A pending assertion of a [Matcher] against a collection of elements.
pub struct AllElementsMatcherCondition<'vdom, D: Driver, M> {
    tester: &'vdom mut DocumentTester<D>,
    selector: Result<D::Selector, TesterError>,
    matcher: M,
}

impl<'vdom, D: Driver, M> AllElementsMatcherCondition<'vdom, D, M>
where
    M: for<'a> Matcher<Vec<ResolvedElement<'a, D>>>,
{
    /// Asserts that the matcher matches the collection immediately, without driving the event
    /// loop.
    pub async fn immediately(self) -> Result<(), TesterError> {
        let Self {
            tester,
            selector,
            matcher,
            ..
        } = self;
        let selector = selector?;
        let driver = &tester.driver;
        check_collection(driver, &selector, &matcher).await
    }
}

impl<'vdom, D: Driver, M> IntoFuture for AllElementsMatcherCondition<'vdom, D, M>
where
    M: for<'a> Matcher<Vec<ResolvedElement<'a, D>>> + 'vdom,
{
    type Output = Result<(), TesterError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + 'vdom>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(wait_for_collection_match(
            self.tester,
            self.selector,
            self.matcher,
        ))
    }
}

async fn check_element<D, M>(
    driver: &D,
    handle: D::NodeHandle,
    matcher: &M,
) -> Result<(), TesterError>
where
    D: Driver,
    M: for<'a> Matcher<ResolvedElement<'a, D>>,
{
    let element = ResolvedElement { handle, driver };
    match matcher.matches(&element).await {
        ControlFlow::Break(_) => Ok(()),
        ControlFlow::Continue(_) => Err(TesterError::AssertionFailure(
            matcher.explain_failure(&element).await,
        )),
    }
}

async fn check_collection<D, M>(
    driver: &D,
    selector: &D::Selector,
    matcher: &M,
) -> Result<(), TesterError>
where
    D: Driver,
    M: for<'a> Matcher<Vec<ResolvedElement<'a, D>>>,
{
    let elements = collect_elements(driver, selector).await;
    match matcher.matches(&elements).await {
        ControlFlow::Break(_) => Ok(()),
        ControlFlow::Continue(_) => Err(TesterError::AssertionFailure(
            matcher.explain_failure(&elements).await,
        )),
    }
}

async fn collect_elements<'a, D: Driver>(
    driver: &'a D,
    selector: &D::Selector,
) -> Vec<ResolvedElement<'a, D>> {
    driver
        .query_all(selector)
        .await
        .into_iter()
        .map(|handle| ResolvedElement { handle, driver })
        .collect()
}

/// Drives the event loop, querying for `selector` and running `matcher` against the result, up
/// to `max_tries()` iterations.
///
/// On final failure, the returned error reflects the state observed at the *last* attempt:
/// `not_found_error` if the element was still missing, or `AssertionFailure` built from
/// `matcher.explain_failure` against the most recent handle. No `pump` runs between the failing
/// `matches` call and the `explain_failure` call, so the description corresponds to the same
/// observation that produced the failure.
async fn wait_for_element_match<'vdom, D, M>(
    tester: &'vdom mut DocumentTester<D>,
    selector: Result<D::Selector, TesterError>,
    not_found_error: TesterError,
    matcher: M,
) -> Result<(), TesterError>
where
    D: Driver,
    M: for<'a> Matcher<ResolvedElement<'a, D>>,
{
    let selector = selector?;
    let max_tries = tester.driver.max_tries();
    let mut tries = 0;
    loop {
        let last_handle = {
            let driver = &tester.driver;
            match driver.query(&selector).await {
                None => None,
                Some(handle) => {
                    let element = ResolvedElement {
                        handle: handle.clone(),
                        driver,
                    };
                    match matcher.matches(&element).await {
                        ControlFlow::Break(_) => return Ok(()),
                        ControlFlow::Continue(_) => Some(handle),
                    }
                }
            }
        };
        tries += 1;
        if tries >= max_tries {
            return match last_handle {
                None => Err(not_found_error),
                Some(handle) => {
                    let element = ResolvedElement {
                        handle,
                        driver: &tester.driver,
                    };
                    Err(TesterError::AssertionFailure(
                        matcher.explain_failure(&element).await,
                    ))
                }
            };
        }
        let _ = tester.driver.pump().await;
    }
}

async fn wait_for_collection_match<'vdom, D, M>(
    tester: &'vdom mut DocumentTester<D>,
    selector: Result<D::Selector, TesterError>,
    matcher: M,
) -> Result<(), TesterError>
where
    D: Driver,
    M: for<'a> Matcher<Vec<ResolvedElement<'a, D>>>,
{
    let selector = selector?;
    let max_tries = tester.driver.max_tries();
    let mut tries = 0;
    loop {
        let matched = {
            let elements = collect_elements(&tester.driver, &selector).await;
            matches!(matcher.matches(&elements).await, ControlFlow::Break(_))
        };
        if matched {
            return Ok(());
        }
        tries += 1;
        if tries >= max_tries {
            let elements = collect_elements(&tester.driver, &selector).await;
            return Err(TesterError::AssertionFailure(
                matcher.explain_failure(&elements).await,
            ));
        }
        let _ = tester.driver.pump().await;
    }
}
