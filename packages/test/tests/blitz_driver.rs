//! End-to-end tests against the in-process [`BlitzDriver`].
//!
//! Doctests already cover the typical `render → query → expect` flow. This file targets paths
//! the doctests don't exercise — the [`Coordinates`] / [`BoundingBox`] layout API, configurable
//! window size, and time advancement.

use dioxus::prelude::*;
use dioxus_core::Event;
use dioxus_html::{Modifiers, PlatformEventData};
use dioxus_native_dom::synthetic_click_event;
use dioxus_test::{by_testid, eq, inner_html, render};
use std::rc::Rc;

#[component]
fn ColoredBox() -> Element {
    rsx! {
        div {
            class: "box",
            style: "width: 100px; height: 50px;",
            "Hello"
        }
    }
}

#[tokio::test]
async fn bounding_box_reports_layout_dimensions() {
    let mut tester = render(ColoredBox)
        .with_window_size(500, 400)
        .build();
    let element = tester.query(".box").await.unwrap();
    let bbox = element.bounding_box().await;
    assert!(
        (bbox.width - 100.0).abs() < 0.5,
        "expected width ~100, got {}",
        bbox.width
    );
    assert!(
        (bbox.height - 50.0).abs() < 0.5,
        "expected height ~50, got {}",
        bbox.height
    );
}

#[tokio::test]
async fn corner_coordinates_track_bounding_box() {
    let mut tester = render(ColoredBox).build();
    let element = tester.query(".box").await.unwrap();
    let bbox = element.bounding_box().await;
    let upper_left = element.upper_left().await;
    let lower_right = element.lower_right().await;
    let center = element.center().await;

    assert_eq!(upper_left.client().x, bbox.x);
    assert_eq!(upper_left.client().y, bbox.y);
    assert_eq!(lower_right.client().x, bbox.x + bbox.width);
    assert_eq!(lower_right.client().y, bbox.y + bbox.height);
    assert_eq!(center.client().x, bbox.x + bbox.width / 2.0);
    assert_eq!(center.client().y, bbox.y + bbox.height / 2.0);
}

#[component]
fn Counter() -> Element {
    let mut count = use_signal(|| 0);
    rsx! {
        button { class: "inc", onclick: move |_| count += 1, "+" }
        div { class: "count", "{count}" }
    }
}

#[tokio::test]
async fn click_and_pump_runs_event_handler() {
    let mut tester = render(Counter).build();
    tester
        .query(".count")
        .expect(inner_html(eq("0")))
        .await
        .unwrap();
    tester.query(".inc").click().await.unwrap();
    tester
        .query(".count")
        .expect(inner_html(eq("1")))
        .await
        .unwrap();
}

// The `ResolvedElement::send_event` method removed in the driver refactor is reachable as
// `tester.driver_mut().send_event(handle, ...)`. This test locks in the new shape so a future
// change to the driver surface doesn't silently strand users of the escape hatch.
#[tokio::test]
async fn send_event_dispatches_through_driver_mut() {
    let mut tester = render(Counter).build();

    let handle = {
        let element = tester.query(".inc").await.unwrap();
        element.handle()
    };

    let event_data = tester
        .driver()
        .with_node(handle, |node| synthetic_click_event(node, Modifiers::empty()));
    let event = Event::new(Rc::new(PlatformEventData::new(event_data)), true);
    tester.driver_mut().send_event(handle, "click", event);

    tester
        .query(".count")
        .expect(inner_html(eq("1")))
        .await
        .unwrap();
}

// Documents the hazard called out in `condition.rs`: awaiting an assertion that the state is
// *unchanged* after an interaction can pass spuriously because the matcher succeeds before the
// event handler runs. The fix is to first await an effect that implies the handler ran, then
// assert on the unchanged state with `.immediately().await`.
#[component]
fn UpdatingLabel() -> Element {
    let mut button_text = use_signal(|| "Click me!");
    let mut label_text = use_signal(|| "Not yet clicked");
    rsx! {
        div { "data-testid": "label", {label_text} }
        button {
            class: "btn",
            onclick: move |_| {
                button_text.set("Don't click again!");
                label_text.set("Now clicked");
            },
            {button_text}
        }
    }
}

#[tokio::test]
async fn awaiting_unchanged_state_can_pass_spuriously() {
    let mut tester = render(UpdatingLabel).build();
    tester.query(".btn").click().await.unwrap();

    // BUG-SHAPED: the matcher passes the first time it's evaluated, before the click's
    // handler has run. The test passes despite the button text *will* change.
    tester
        .query(".btn")
        .expect(inner_html(eq("Click me!")))
        .await
        .unwrap();
}

#[tokio::test]
async fn awaiting_a_committed_effect_first_avoids_spurious_pass() {
    let mut tester = render(UpdatingLabel).build();
    tester.query(".btn").click().await.unwrap();

    // Wait until the handler-driven label change is observable...
    tester
        .query(by_testid("label"))
        .expect(inner_html(eq("Now clicked")))
        .await
        .unwrap();

    // ...then assert on the button text immediately. If the button text were unchanged this
    // would pass; because it actually changed, it correctly fails.
    let result = tester
        .query(".btn")
        .expect(inner_html(eq("Click me!")))
        .immediately()
        .await;
    assert!(
        result.is_err(),
        "expected the immediate assertion on the stale button text to fail"
    );
}
