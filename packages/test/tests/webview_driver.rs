use dioxus::prelude::*;
use dioxus_test::{
    TestElement, WebSysDriver, by_testid, contains_string, inner_html, render_with_driver,
};

fn main() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run_test());
}

async fn run_test() {
    let mut tester = render_with_driver::<WebSysDriver>(Counter)
        .with_window_size(320, 240)
        .build();

    tester
        .query(by_testid("count"))
        .expect(inner_html(contains_string("Count: 0")))
        .immediately()
        .unwrap();

    tester.query("button").click().await.unwrap();

    tester
        .query(by_testid("count"))
        .expect(inner_html(contains_string("Count: 1")))
        .await
        .unwrap();

    assert!(tester.root().inner_html().contains("Count: 1"));
}

#[component]
fn Counter() -> Element {
    let mut count = use_signal(|| 0);

    rsx! {
        div {
            button {
                onclick: move |_| count += 1,
                "Increment"
            }
            div {
                "data-testid": "count",
                "Count: {count}"
            }
        }
    }
}
