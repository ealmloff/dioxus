#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::test_attr_in_doctest)] // The doctests need to show examples of tests
//! A testing crate for Dioxus.
//!
//! This crate facilitates rendering, interacting with, and querying the DOM in tests of Dioxus
//! apps. Tests have fairly precise control over both the rendering lifecycle and asynchronous
//! operations. Thus they can assert both on the final outcome of interactions, such as the
//! rendered data obtained from a call to a backend, as well as intermediate states, such as the
//! presence of a spinner while loading data.
//!
//! ## Drivers
//!
//! The DOM is driven by an implementation of the [Driver] trait. The default [BlitzDriver] uses
//! [Blitz](https://crates.io/crates/blitz) in-process — no browser required, but layout-completeness
//! is limited to what Blitz supports.
//!
//! The trait is the integration point for **Playwright-style** backends: implementing [Driver] over
//! a CDP or WebDriver client lets the same test API drive a real browser. Element handles
//! ([ResolvedElement]) and matchers are written against the trait, so test code is portable across
//! backends.
//!
//! Tests operate "headless"; the in-process backend cannot render to the screen.
//!
//! ## Usage
//!
//! Tests can construct a [DocumentTester] instance to render and interact with the DOM. To
//! construct a [DocumentTester], invoke [render] on a Dioxus component, then `build` to trigger
//! the initial layout. The tester provides methods for querying elements by CSS selector or by
//! test ID.
//!
//! ```
//! use dioxus::prelude::*;
//! use dioxus_test::{eq, inner_html, render};
//!
//! #[component]
//! fn MyComponent() -> Element {
//!     rsx! {
//!         div {
//!              class: "test-component",
//!              "Hello, world!"
//!         }
//!     }
//! }
//!
//! # /* Make sure this also compiles as a doctest.
//! #[tokio::test]
//! # */
//! async fn my_component_renders_correctly() {
//!     let mut tester = render(MyComponent).build();
//!     tester
//!         .query(".test-component")
//!         .expect(inner_html(eq("Hello, world!")))
//!         .await
//!         .unwrap();
//! }
//! # tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(my_component_renders_correctly());
//! ```
//!
//! Assertions can be awaited asynchronously, allowing asynchronous operations to run and the DOM
//! to evolve. The tester keeps checking whether the assertion is true and handing control back to
//! the async runtime until the assertion is true, or a maximum number of tries is reached.
//!
//! ```
//! use dioxus::prelude::*;
//! use dioxus_test::{eq, inner_html, render};
//!
//! #[component]
//! fn MyComponent() -> Element {
//!     let mut text = use_signal(|| "Click me!");
//!     rsx! {
//!         button {
//!              class: "test-button",
//!              onclick: move |_| {
//!                  *text.write() = "Don't click any more!";
//!              },
//!              {text}
//!         }
//!     }
//! }
//!
//! # /* Make sure this also compiles as a doctest.
//! #[tokio::test]
//! # */
//! async fn my_component_changes_button_text_on_click() {
//!     let mut tester = render(MyComponent).build();
//!     tester.query(".test-button").click().await.unwrap();
//!     tester
//!         .query(".test-button")
//!         .expect(inner_html(eq("Don't click any more!")))
//!         .await
//!         .unwrap();
//! }
//! # tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(my_component_changes_button_text_on_click());
//! ```
//!
//! ## Asynchronous operations
//!
//! For more precise control over DOM evolution, one can use [DocumentTester::pump]. This returns
//! control to the async runtime and thus drives any asynchronous operations such as requests to
//! the backend.
//!
//! ```
//! use dioxus::prelude::*;
//! use dioxus_test::{eq, inner_html, render};
//!
//! #[component]
//! fn MyComponent() -> Element {
//!     let mut text = use_signal(|| "Click me!");
//!     rsx! {
//!         button {
//!              class: "test-button",
//!              onclick: move |_| {
//!                  *text.write() = "Don't click any more!";
//!              },
//!              {text}
//!         }
//!     }
//! }
//!
//! #[tokio::test]
//! async fn my_component_changes_button_text_on_click() {
//!     let mut tester = render(MyComponent).build();
//!     tester.query(".test-button").click().await.unwrap();
//!     let _ = tester.pump().await;
//!     tester
//!         .query(".test-button")
//!         .expect(inner_html(eq("Don't click any more!")))
//!         .immediately()
//!         .await
//!         .unwrap();
//! }
//! ```
//!
//! ## Limitations
//!
//! Interactions with the DOM operate directly on elements, not on the screen. If the test
//! dispatches a click on an element which is visually covered, the element responds as though it
//! were reachable.
//!
//! The in-process [BlitzDriver]'s layout is limited by what Blitz supports.

mod blitz_driver;
mod condition;
mod document;
mod driver;
mod element;
mod matcher;
mod result;

pub use blitz_driver::{BlitzDriver, BlitzNodeHandle, DEFAULT_MAX_TRIES, DEFAULT_PUMP_TIMEOUT};
pub use condition::{AllElementsCondition, ElementCondition};
pub use document::{DocumentTester, TryIntoSelector, by_testid, render};
pub use driver::{BoundingBox, Driver, PumpTimeout};
pub use element::ResolvedElement;
pub use matcher::{Matcher, contains_string, empty, eq, inner_html, not};
pub use result::{Result, TesterError};
