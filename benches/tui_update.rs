use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use dioxus::prelude::*;
use dioxus_tui::{Config, TuiContext};

criterion_group!(mbenches, tui_update);
criterion_main!(mbenches);

/// This benchmarks the cache performance of the TUI for small edits by changing one box at a time.
fn tui_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("Update boxes");

    // We can also use loops to define multiple benchmarks, even over multiple dimensions.
    for size in 1..=8u32 {
        let parameter_string = format!("{}", (3 * size).pow(2));
        group.bench_with_input(
            BenchmarkId::new("size", parameter_string),
            &size,
            |b, size| {
                b.iter(|| match size {
                    1 => dioxus::tui::launch_cfg(app3, Config::default()),
                    2 => dioxus::tui::launch_cfg(app6, Config::default()),
                    3 => dioxus::tui::launch_cfg(app9, Config::default()),
                    4 => dioxus::tui::launch_cfg(app12, Config::default()),
                    5 => dioxus::tui::launch_cfg(app15, Config::default()),
                    6 => dioxus::tui::launch_cfg(app18, Config::default()),
                    7 => dioxus::tui::launch_cfg(app21, Config::default()),
                    8 => dioxus::tui::launch_cfg(app24, Config::default()),
                    _ => (),
                })
            },
        );
    }
}

#[derive(Props, PartialEq)]
struct BoxProps {
    x: usize,
    y: usize,
    size: usize,
    offset: usize,
}
#[allow(non_snake_case)]
fn Box(cx: Scope<BoxProps>) -> Element {
    let count = use_state(&cx, || cx.props.offset);

    let x = cx.props.x * 2;
    let y = cx.props.y * 2;

    let ctx: TuiContext = cx.consume_context().unwrap();
    let size = cx.props.size;
    let offset = cx.props.offset;

    use_future(&cx, (), move |_| {
        let count = count.clone();
        let ctx = ctx.clone();
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(offset as u64)).await;
            if *count.get() + 1 >= (size * size) {
                ctx.quit();
            } else {
                count.with_mut(|i| {
                    *i += 1;
                    *i = *i % (size * size);
                });
            }
        }
    });

    let count = count.get();
    let hue = (count * 10) % 255;
    cx.render(rsx! {
        div {
            left: "{x}%",
            top: "{y}%",
            width: "100%",
            height: "100%",
            background_color: "hsl({hue}, 100%, 50%)",
            align_items: "center",
            p{"{hue:03}"}
        }
    })
}

#[derive(Props, PartialEq)]
struct GridProps {
    size: usize,
}
#[allow(non_snake_case)]
fn Grid(cx: Scope<GridProps>) -> Element {
    let size = cx.props.size;

    cx.render(rsx! {
        div{
            width: "100%",
            height: "100%",
            flex_direction: "column",
            (0..size).map(|x|
                    {
                    cx.render(rsx! {
                        div{
                            width: "100%",
                            height: "100%",
                            flex_direction: "row",
                            (0..size).map(|y|
                                {
                                    let key = format!("{}-{}", x, y);
                                    cx.render(rsx! {
                                        Box{
                                            x: x,
                                            y: y,
                                            key: "{key}",
                                            size: size,
                                            offset: y*size + x,
                                        }
                                    })
                                }
                            )
                        }
                    })
                }
            )
        }
    })
}

fn app3(cx: Scope) -> Element {
    cx.render(rsx! {
        div{
            width: "100%",
            height: "100%",
            Grid{
                size: 3,
            }
        }
    })
}

fn app6(cx: Scope) -> Element {
    cx.render(rsx! {
        div{
            width: "100%",
            height: "100%",
            Grid{
                size: 6,
            }
        }
    })
}

fn app9(cx: Scope) -> Element {
    cx.render(rsx! {
        div{
            width: "100%",
            height: "100%",
            Grid{
                size: 9,
            }
        }
    })
}

fn app12(cx: Scope) -> Element {
    cx.render(rsx! {
        div{
            width: "100%",
            height: "100%",
            Grid{
                size: 12,
            }
        }
    })
}

fn app15(cx: Scope) -> Element {
    cx.render(rsx! {
        div{
            width: "100%",
            height: "100%",
            Grid{
                size: 15,
            }
        }
    })
}

fn app18(cx: Scope) -> Element {
    cx.render(rsx! {
        div{
            width: "100%",
            height: "100%",
            Grid{
                size: 18,
            }
        }
    })
}

fn app21(cx: Scope) -> Element {
    cx.render(rsx! {
        div{
            width: "100%",
            height: "100%",
            Grid{
                size: 21,
            }
        }
    })
}

fn app24(cx: Scope) -> Element {
    cx.render(rsx! {
        div{
            width: "100%",
            height: "100%",
            Grid{
                size: 24,
            }
        }
    })
}
