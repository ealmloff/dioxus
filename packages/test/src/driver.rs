use crate::TesterError;

/// A backend capable of rendering, querying, and interacting with a DOM in tests.
///
/// Implementations provide the operations [crate::DocumentTester] uses to drive a test. The
/// built-in [crate::BlitzDriver] runs the DOM in-process using Blitz; to drive a remote renderer
/// such as a browser via CDP or WebDriver, implement this trait over that transport.
///
/// All operations except [Self::pump] take `&self` so that multiple [crate::ResolvedElement]
/// references over the same driver can coexist (e.g. inside collection matchers). Implementations
/// of remote backends will typically need internal synchronization (a [`std::sync::Mutex`] or an
/// async queue) for the shared-borrow read methods.
///
/// Each backend defines its own opaque [Self::NodeHandle] and its own [Self::Selector].
pub trait Driver: 'static {
    /// An opaque handle to a node managed by this driver.
    ///
    /// `Clone` rather than `Copy` so backends can use non-`Copy` representations — e.g. a
    /// WebDriver implementation that stores element references as `Arc<str>` or `String`. The
    /// crate clones handles only on the rare path where the same handle is reused after a
    /// fallible match (see [`crate::ElementCondition`]), so the cost is negligible.
    type NodeHandle: Clone + std::fmt::Debug;

    /// A parsed selector ready for repeated use with [Self::query] / [Self::query_all].
    ///
    /// The `'static` bound rules out selectors that borrow from a parser arena. In exchange,
    /// [`crate::ElementCondition`] can cache a parsed selector and reuse it across pump
    /// iterations without lifetime gymnastics. Backends that produce arena-allocated selectors
    /// should clone them into an owned representation before returning.
    type Selector: 'static;

    /// Parses the given CSS selector string.
    ///
    /// Returns [TesterError::InvalidCssSelector] if the string is not a valid selector. The
    /// returned selector remains valid for the lifetime of this driver.
    fn parse_selector(&self, selector: &str) -> Result<Self::Selector, TesterError>;

    /// The maximum number of pump iterations the tester will perform while waiting for an
    /// element or an assertion to hold.
    fn max_tries(&self) -> usize;

    /// Returns a handle to the root node of the DOM.
    fn root(&self) -> impl Future<Output = Self::NodeHandle>;

    /// Returns the first node matching the given selector, if any.
    fn query(
        &self,
        selector: &Self::Selector,
    ) -> impl Future<Output = Option<Self::NodeHandle>>;

    /// Returns all nodes matching the given selector.
    fn query_all(
        &self,
        selector: &Self::Selector,
    ) -> impl Future<Output = Vec<Self::NodeHandle>>;

    /// Returns the inner HTML of the given node.
    fn inner_html(&self, node: Self::NodeHandle) -> impl Future<Output = String>;

    /// Returns the outer HTML of the given node.
    fn outer_html(&self, node: Self::NodeHandle) -> impl Future<Output = String>;

    /// Returns the layout-resolved bounding box of the given node.
    fn bounding_box(&self, node: Self::NodeHandle) -> impl Future<Output = BoundingBox>;

    /// Dispatches a click on the given node.
    fn click(&self, node: Self::NodeHandle) -> impl Future<Output = ()>;

    /// Drives one round of asynchronous work.
    ///
    /// For an in-process backend this typically pumps the framework's event loop. For a remote
    /// backend this typically waits for the page to settle.
    ///
    /// Returns [`Err(PumpTimeout)`](PumpTimeout) if the driver concluded the work-wait phase by
    /// timing out rather than receiving an event. This is informational — a single elapsed pump
    /// is normal inside a polling loop, so internal waiters discard it.
    fn pump(&mut self) -> impl Future<Output = Result<(), PumpTimeout>>;
}

// Operations that vary too much between backends — clock advancement, context provision,
// non-`click` synthetic events — are intentionally not on this trait. Concrete drivers expose
// them as inherent methods reachable through [`crate::DocumentTester::driver_mut`]. See
// [`crate::BlitzDriver::advance_time`], [`crate::BlitzDriver::provide_root_context`], and
// [`crate::BlitzDriver::send_event`].

/// Returned by [`Driver::pump`] when the driver gave up waiting for the next round of work.
///
/// Not a hard error — most callers can ignore it with `let _ = tester.pump().await;`. Tests that
/// want to assert that some async work happened can match on it explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PumpTimeout;

impl std::fmt::Display for PumpTimeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pump timed out waiting for asynchronous work")
    }
}

impl std::error::Error for PumpTimeout {}

/// The bounding box of a DOM node in CSS pixels, relative to the document origin.
///
/// Returned by [Driver::bounding_box] and [crate::ResolvedElement::bounding_box].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl BoundingBox {
    /// The (x, y) coordinates of the centre of the box.
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// The (x, y) coordinates of the upper-left corner.
    pub fn upper_left(&self) -> (f64, f64) {
        (self.x, self.y)
    }

    /// The (x, y) coordinates of the upper-right corner.
    pub fn upper_right(&self) -> (f64, f64) {
        (self.x + self.width, self.y)
    }

    /// The (x, y) coordinates of the lower-left corner.
    pub fn lower_left(&self) -> (f64, f64) {
        (self.x, self.y + self.height)
    }

    /// The (x, y) coordinates of the lower-right corner.
    pub fn lower_right(&self) -> (f64, f64) {
        (self.x + self.width, self.y + self.height)
    }

    /// The (width, height) of the box.
    pub fn size(&self) -> (f64, f64) {
        (self.width, self.height)
    }
}
