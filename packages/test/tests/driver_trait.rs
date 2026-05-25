//! Validates that the [`Driver`] trait is implementable from outside the crate, and exercises
//! the wait/expect plumbing in `condition.rs` against a deterministic backend without involving
//! Blitz layout.

use dioxus_test::{
    BoundingBox, DocumentTester, Driver, PumpTimeout, TesterError, contains_string, empty, eq,
    inner_html, not,
};
use std::cell::RefCell;

#[derive(Debug, Clone)]
struct MockNode {
    id: u32,
    key: String,
    inner_html: String,
    outer_html: String,
    bbox: BoundingBox,
}

#[derive(Debug, Clone)]
struct MockSelector(String);

struct MockDriver {
    nodes: RefCell<Vec<MockNode>>,
    delayed: RefCell<Vec<(usize, MockNode)>>,
    clicks: RefCell<Vec<u32>>,
    max_tries: usize,
}

impl MockDriver {
    fn new() -> Self {
        Self {
            nodes: RefCell::new(vec![]),
            delayed: RefCell::new(vec![]),
            clicks: RefCell::new(vec![]),
            max_tries: 5,
        }
    }

    fn add_node(&self, node: MockNode) {
        self.nodes.borrow_mut().push(node);
    }

    fn schedule(&self, pumps_until_visible: usize, node: MockNode) {
        self.delayed
            .borrow_mut()
            .push((pumps_until_visible, node));
    }

    fn clicks(&self) -> Vec<u32> {
        self.clicks.borrow().clone()
    }
}

impl Driver for MockDriver {
    type NodeHandle = u32;
    type Selector = MockSelector;

    fn parse_selector(&self, selector: &str) -> Result<Self::Selector, TesterError> {
        if selector.starts_with('!') {
            return Err(TesterError::InvalidCssSelector(format!(
                "Invalid CSS selector '{selector}'"
            )));
        }
        Ok(MockSelector(selector.to_string()))
    }

    fn max_tries(&self) -> usize {
        self.max_tries
    }

    async fn root(&self) -> u32 {
        0
    }

    async fn query(&self, selector: &Self::Selector) -> Option<u32> {
        self.nodes
            .borrow()
            .iter()
            .find(|n| n.key == selector.0)
            .map(|n| n.id)
    }

    async fn query_all(&self, selector: &Self::Selector) -> Vec<u32> {
        self.nodes
            .borrow()
            .iter()
            .filter(|n| n.key == selector.0)
            .map(|n| n.id)
            .collect()
    }

    async fn inner_html(&self, handle: u32) -> String {
        self.nodes
            .borrow()
            .iter()
            .find(|n| n.id == handle)
            .map(|n| n.inner_html.clone())
            .unwrap_or_default()
    }

    async fn outer_html(&self, handle: u32) -> String {
        self.nodes
            .borrow()
            .iter()
            .find(|n| n.id == handle)
            .map(|n| n.outer_html.clone())
            .unwrap_or_default()
    }

    async fn bounding_box(&self, handle: u32) -> BoundingBox {
        self.nodes
            .borrow()
            .iter()
            .find(|n| n.id == handle)
            .map(|n| n.bbox)
            .unwrap_or_default()
    }

    async fn click(&self, handle: u32) {
        self.clicks.borrow_mut().push(handle);
    }

    async fn pump(&mut self) -> Result<(), PumpTimeout> {
        let mut delayed = self.delayed.borrow_mut();
        let mut still_pending = Vec::with_capacity(delayed.len());
        for (remaining, node) in delayed.drain(..) {
            if remaining == 0 {
                self.nodes.borrow_mut().push(node);
            } else {
                still_pending.push((remaining - 1, node));
            }
        }
        *delayed = still_pending;
        Ok(())
    }
}

fn node(id: u32, key: &str, inner: &str) -> MockNode {
    MockNode {
        id,
        key: key.into(),
        inner_html: inner.into(),
        outer_html: format!("<{key}>{inner}</{key}>"),
        bbox: BoundingBox {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
    }
}

fn build_tester(driver: MockDriver) -> DocumentTester<MockDriver> {
    DocumentTester::with_driver(driver)
}

#[tokio::test]
async fn parse_selector_error_propagates_through_query() {
    let mut tester = build_tester(MockDriver::new());
    let err = tester.query("!bad").await.unwrap_err();
    assert!(
        matches!(err, TesterError::InvalidCssSelector(_)),
        "expected InvalidCssSelector, got {err:?}",
    );
}

#[tokio::test]
async fn parse_selector_error_propagates_through_query_all() {
    let mut tester = build_tester(MockDriver::new());
    let err = tester.query_all("!bad").expect(empty()).await.unwrap_err();
    assert!(
        matches!(err, TesterError::InvalidCssSelector(_)),
        "expected InvalidCssSelector, got {err:?}",
    );
}

#[tokio::test]
async fn query_resolves_immediately_when_present() {
    let driver = MockDriver::new();
    driver.add_node(node(1, "button", "Hello"));
    let mut tester = build_tester(driver);

    let element = tester.query("button").await.unwrap();
    assert_eq!(element.handle(), 1);
}

#[tokio::test]
async fn query_waits_across_pumps_for_node_to_appear() {
    let driver = MockDriver::new();
    driver.schedule(2, node(7, "div", "later"));
    let mut tester = build_tester(driver);

    let element = tester.query("div").await.unwrap();
    assert_eq!(element.handle(), 7);
}

#[tokio::test]
async fn query_returns_not_found_after_max_tries() {
    let mut tester = build_tester(MockDriver::new());
    let err = tester.query(".missing").await.unwrap_err();
    assert!(
        matches!(err, TesterError::NoSuchElementWithCssSelector(_)),
        "expected NoSuchElementWithCssSelector, got {err:?}",
    );
}

#[tokio::test]
async fn expect_inner_html_matches_immediately() {
    let driver = MockDriver::new();
    driver.add_node(node(1, "span", "ok"));
    let mut tester = build_tester(driver);

    tester
        .query("span")
        .expect(inner_html(eq("ok")))
        .await
        .unwrap();
}

#[tokio::test]
async fn expect_inner_html_failure_reports_actual_value() {
    let driver = MockDriver::new();
    driver.add_node(node(1, "span", "actual"));
    let mut tester = build_tester(driver);

    let err = tester
        .query("span")
        .expect(inner_html(eq("expected")))
        .await
        .unwrap_err();
    let TesterError::AssertionFailure(msg) = err else {
        panic!("expected AssertionFailure, got {err:?}");
    };
    assert!(msg.contains("inner HTML"), "missing describe: {msg}");
    assert!(msg.contains("actual"), "missing actual value: {msg}");
}

#[tokio::test]
async fn expect_contains_string_matches() {
    let driver = MockDriver::new();
    driver.add_node(node(1, "p", "hello, world"));
    let mut tester = build_tester(driver);

    tester
        .query("p")
        .expect(inner_html(contains_string("world")))
        .await
        .unwrap();
}

#[tokio::test]
async fn expect_not_inverts_matcher() {
    let driver = MockDriver::new();
    driver.add_node(node(1, "p", "abc"));
    let mut tester = build_tester(driver);

    tester
        .query("p")
        .expect(inner_html(not(eq("xyz"))))
        .await
        .unwrap();
}

#[tokio::test]
async fn query_all_empty_matches_empty_collection() {
    let mut tester = build_tester(MockDriver::new());
    tester.query_all(".none").expect(empty()).await.unwrap();
}

#[tokio::test]
async fn query_all_non_empty_with_not_empty_matches() {
    let driver = MockDriver::new();
    driver.add_node(node(1, "li", "a"));
    driver.add_node(node(2, "li", "b"));
    let mut tester = build_tester(driver);

    tester
        .query_all("li")
        .expect(not(empty()))
        .await
        .unwrap();
}

#[tokio::test]
async fn click_dispatches_to_driver() {
    let driver = MockDriver::new();
    driver.add_node(node(42, "button", "click me"));
    let mut tester = build_tester(driver);

    tester.query("button").click().await.unwrap();
    let driver_ref = tester.driver();
    assert_eq!(driver_ref.clicks(), vec![42]);
}

#[tokio::test]
async fn root_returns_handle_zero() {
    let tester = build_tester(MockDriver::new());
    let root = tester.root().await;
    assert_eq!(root.handle(), 0);
}

#[tokio::test]
async fn pump_timeout_is_returnable_from_custom_driver() {
    struct AlwaysTimeoutDriver;

    impl Driver for AlwaysTimeoutDriver {
        type NodeHandle = u32;
        type Selector = ();
        fn parse_selector(&self, _: &str) -> Result<(), TesterError> {
            Ok(())
        }
        fn max_tries(&self) -> usize {
            1
        }
        async fn root(&self) -> u32 {
            0
        }
        async fn query(&self, _: &()) -> Option<u32> {
            None
        }
        async fn query_all(&self, _: &()) -> Vec<u32> {
            vec![]
        }
        async fn inner_html(&self, _: u32) -> String {
            String::new()
        }
        async fn outer_html(&self, _: u32) -> String {
            String::new()
        }
        async fn bounding_box(&self, _: u32) -> BoundingBox {
            BoundingBox::default()
        }
        async fn click(&self, _: u32) {}
        async fn pump(&mut self) -> Result<(), PumpTimeout> {
            Err(PumpTimeout)
        }
    }

    let mut tester: DocumentTester<AlwaysTimeoutDriver> =
        DocumentTester::with_driver(AlwaysTimeoutDriver);
    assert_eq!(tester.pump().await, Err(PumpTimeout));
}
