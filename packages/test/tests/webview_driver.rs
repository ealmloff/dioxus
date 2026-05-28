use dioxus::prelude::*;
use dioxus_test::{
    TestElement, WebSysDriver, by_testid, contains_string, empty, inner_html, not,
    render_with_driver,
};

fn main() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run_test());
}

async fn run_test() {
    let mut tester = render_with_driver::<WebSysDriver>(Inventory)
        .with_window_size(420, 320)
        .build();

    tester
        .query(by_testid("count"))
        .expect(inner_html(contains_string("Items: 2")))
        .immediately()
        .unwrap();
    tester
        .query(by_testid("detail-empty"))
        .expect(inner_html(contains_string("No selection")))
        .immediately()
        .unwrap();
    tester
        .query_all(by_testid("item"))
        .expect(not(empty()))
        .immediately()
        .unwrap();

    {
        let item_query = tester.query_all(by_testid("item"));
        let items = item_query.immediately();
        assert_eq!(items.len(), 2);
        assert!(items[0].inner_html().contains("Alpha"));
        assert!(items[1].inner_html().contains("Beta"));
    }

    tester.query(by_testid("add")).click().await.unwrap();
    tester.query(by_testid("add")).click().await.unwrap();

    tester
        .query(by_testid("count"))
        .expect(inner_html(contains_string("Items: 4")))
        .await
        .unwrap();

    {
        let item_query = tester.query_all(by_testid("item"));
        let items = item_query.immediately();
        assert_eq!(items.len(), 4);
        assert!(items[2].inner_html().contains("Gamma"));
        assert!(items[3].inner_html().contains("Delta"));
    }

    tester.query(by_testid("item-2")).click().await.unwrap();
    tester
        .query(by_testid("detail"))
        .expect(inner_html(contains_string("Selected Gamma")))
        .await
        .unwrap();
    tester
        .query(by_testid("status"))
        .expect(inner_html(contains_string("Position 3 of 4")))
        .immediately()
        .unwrap();

    tester.query(by_testid("remove")).click().await.unwrap();
    tester
        .query(by_testid("count"))
        .expect(inner_html(contains_string("Items: 3")))
        .await
        .unwrap();
    tester
        .query_all(by_testid("detail"))
        .expect(empty())
        .immediately()
        .unwrap();
    assert!(!tester.root().inner_html().contains("Gamma"));

    tester.query(by_testid("clear")).click().await.unwrap();
    tester
        .query_all(by_testid("item"))
        .expect(empty())
        .await
        .unwrap();
    tester
        .query(by_testid("empty-state"))
        .expect(inner_html(contains_string("Inventory empty")))
        .immediately()
        .unwrap();
    assert!(tester.root().inner_html().contains("Items: 0"));
}

#[component]
fn Inventory() -> Element {
    let mut items = use_signal(|| vec!["Alpha".to_string(), "Beta".to_string()]);
    let mut selected = use_signal(|| None::<usize>);

    let items_snapshot = items.read().clone();
    let selected_index = *selected.read();
    let selected_item = selected_index.and_then(|index| {
        items_snapshot
            .get(index)
            .map(|item| (index, item.to_string()))
    });

    rsx! {
        section {
            "data-testid": "inventory",
            h1 { "Inventory" }
            div {
                "data-testid": "count",
                "Items: {items_snapshot.len()}"
            }
            div {
                button {
                    "data-testid": "add",
                    onclick: move |_| {
                        let next_name = match items.read().len() {
                            0 => "Alpha",
                            1 => "Beta",
                            2 => "Gamma",
                            3 => "Delta",
                            _ => "Extra",
                        };
                        items.write().push(next_name.to_string());
                    },
                    "Add item"
                }
                button {
                    "data-testid": "remove",
                    onclick: move |_| {
                        let selected_index = *selected.read();
                        if let Some(index) = selected_index {
                            let mut items = items.write();
                            if index < items.len() {
                                items.remove(index);
                            }
                            selected.set(None);
                        }
                    },
                    "Remove selected"
                }
                button {
                    "data-testid": "clear",
                    onclick: move |_| {
                        items.write().clear();
                        selected.set(None);
                    },
                    "Clear"
                }
            }
            if items_snapshot.is_empty() {
                p {
                    "data-testid": "empty-state",
                    "Inventory empty"
                }
            } else {
                ul {
                    for (index, item) in items_snapshot.iter().enumerate() {
                        li {
                            key: "{item}",
                            class: "item",
                            "data-testid": "item",
                            "data-index": "{index}",
                            button {
                                "data-testid": "item-{index}",
                                onclick: move |_| selected.set(Some(index)),
                                "{item}"
                            }
                        }
                    }
                }
            }
            if let Some((index, item)) = selected_item {
                aside {
                    "data-testid": "detail",
                    h2 { "Selected {item}" }
                    p {
                        "data-testid": "status",
                        "Position {index + 1} of {items_snapshot.len()}"
                    }
                }
            } else {
                p {
                    "data-testid": "detail-empty",
                    "No selection"
                }
            }
        }
    }
}
