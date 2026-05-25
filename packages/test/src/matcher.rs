use crate::driver::Driver;
use crate::element::ResolvedElement;
use std::ops::ControlFlow;

/// A condition on a value of type `T`.
///
/// Matchers are `async` so adapters that fetch data through the [Driver] (e.g. [inner_html]) work
/// uniformly across in-process and remote backends.
///
/// For matchers that don't need async work (e.g. [eq], [contains_string], [empty]), the future
/// resolves synchronously on first poll — the cost over a plain `fn` is a wrapper future, which
/// the compiler can usually flatten through the surrounding `async fn`.
///
/// `matches` and `explain_failure` both take `&T` so a caller (e.g. [`crate::ElementCondition`])
/// that resolves an element can pass the same borrow to both methods without re-querying the
/// driver.
pub trait Matcher<T: std::fmt::Debug> {
    /// Returns [ControlFlow::Break] if `actual` matches, [ControlFlow::Continue] otherwise.
    fn matches(&self, actual: &T) -> impl Future<Output = ControlFlow<()>>;

    /// A short description of the expected condition, used in failure messages.
    fn describe(&self) -> String;

    /// Builds a failure description for `actual`.
    fn explain_failure(&self, actual: &T) -> impl Future<Output = String> {
        async move { format!("\nExpected: {}\n  but was: {actual:?}\n", self.describe()) }
    }
}

/// Adapter that turns a [Matcher] on a `String` into a matcher on a [ResolvedElement] which
/// fetches the element's inner HTML and forwards it.
pub struct InnerHtmlMatcher<I>(I);

impl<'a, I, D> Matcher<ResolvedElement<'a, D>> for InnerHtmlMatcher<I>
where
    I: Matcher<String>,
    D: Driver,
{
    async fn matches(&self, element: &ResolvedElement<'a, D>) -> ControlFlow<()> {
        let html = element.inner_html().await;
        self.0.matches(&html).await
    }

    fn describe(&self) -> String {
        format!("inner HTML {}", self.0.describe())
    }

    async fn explain_failure(&self, element: &ResolvedElement<'a, D>) -> String {
        let html = element.inner_html().await;
        format!(
            "\nExpected: inner HTML {}\n  but was: {html:?}\n",
            self.0.describe()
        )
    }
}

/// Returns a [Matcher] which matches a [ResolvedElement] whose inner HTML is matched by `inner`.
pub fn inner_html<I: Matcher<String>>(inner: I) -> InnerHtmlMatcher<I> {
    InnerHtmlMatcher(inner)
}

/// Matcher returned by [eq].
pub struct EqMatcher<T>(T);

impl<T, A> Matcher<A> for EqMatcher<T>
where
    T: std::fmt::Debug,
    A: PartialEq<T> + std::fmt::Debug,
{
    async fn matches(&self, actual: &A) -> ControlFlow<()> {
        if actual == &self.0 {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }

    fn describe(&self) -> String {
        format!("equal to {:?}", self.0)
    }
}

/// Returns a [Matcher] which matches a value equal to `value` in the sense of [`PartialEq`].
pub fn eq<T: std::fmt::Debug>(value: T) -> EqMatcher<T> {
    EqMatcher(value)
}

/// Matcher returned by [contains_string].
pub struct ContainsStringMatcher<E>(E);

impl<E: AsRef<str>> Matcher<String> for ContainsStringMatcher<E> {
    async fn matches(&self, actual: &String) -> ControlFlow<()> {
        if actual.contains(self.0.as_ref()) {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }

    fn describe(&self) -> String {
        format!("contains string {}", self.0.as_ref())
    }
}

/// Returns a [Matcher] which matches a `String` containing the given substring.
pub fn contains_string<E: AsRef<str>>(substring: E) -> ContainsStringMatcher<E> {
    ContainsStringMatcher(substring)
}

/// Matcher returned by [not].
pub struct NotMatcher<I>(I);

impl<T, I> Matcher<T> for NotMatcher<I>
where
    T: std::fmt::Debug,
    I: Matcher<T>,
{
    async fn matches(&self, actual: &T) -> ControlFlow<()> {
        match self.0.matches(actual).await {
            ControlFlow::Continue(_) => ControlFlow::Break(()),
            ControlFlow::Break(_) => ControlFlow::Continue(()),
        }
    }

    fn describe(&self) -> String {
        format!("not {}", self.0.describe())
    }
}

/// Returns a [Matcher] which matches any data not matched by `inner`.
pub fn not<I>(inner: I) -> NotMatcher<I> {
    NotMatcher(inner)
}

/// Matcher returned by [empty].
pub struct EmptyMatcher;

impl<T: std::fmt::Debug> Matcher<Vec<T>> for EmptyMatcher {
    async fn matches(&self, actual: &Vec<T>) -> ControlFlow<()> {
        if actual.is_empty() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }

    fn describe(&self) -> String {
        "an empty collection".into()
    }
}

/// Returns a [Matcher] which matches an empty collection.
pub fn empty() -> EmptyMatcher {
    EmptyMatcher
}
