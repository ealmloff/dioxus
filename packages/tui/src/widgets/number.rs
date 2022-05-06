use dioxus_core as dioxus;
use dioxus_core::prelude::fc_to_builder;
use dioxus_core::VNode;
use dioxus_core::*;
use dioxus_core_macro::*;
use dioxus_elements::KeyCode;
use dioxus_hooks::*;
use dioxus_html as dioxus_elements;
use dioxus_html::on::FormData;
use dioxus_native_core::utils::cursor::Cursor;
use std::collections::HashMap;

use crate::widgets::Input;

#[derive(Props)]
pub(crate) struct NumbericInputProps<'a> {
    #[props(!optional)]
    raw_oninput: Option<&'a EventHandler<'a, FormData>>,
    #[props(!optional)]
    value: Option<&'a str>,
    #[props(!optional)]
    size: Option<&'a str>,
    #[props(!optional)]
    width: Option<&'a str>,
    #[props(!optional)]
    height: Option<&'a str>,
}
#[allow(non_snake_case)]
pub(crate) fn NumbericInput<'a>(cx: Scope<'a, NumbericInputProps>) -> Element<'a> {
    let text_ref = use_ref(&cx, || {
        if let Some(intial_text) = cx.props.value {
            intial_text.to_string()
        } else {
            String::new()
        }
    });
    let cursor = use_ref(&cx, || Cursor::default());

    let text = text_ref.read().clone();
    let start_highlight = cursor.read().first().idx(&text);
    let end_highlight = cursor.read().last().idx(&text);
    let (text_before_first_cursor, text_after_first_cursor) = text.split_at(start_highlight);
    let (text_highlighted, text_after_second_cursor) =
        text_after_first_cursor.split_at(end_highlight - start_highlight);

    let text_highlighted = if text_highlighted.is_empty() {
        String::new()
    } else {
        text_highlighted.to_string() + "|"
    };

    let max_len = cx
        .props
        .size
        .as_ref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(usize::MAX);

    let width = cx.props.width.unwrap_or("10px");
    let height = cx.props.height.unwrap_or("3px");

    // don't draw a border unless there is enough space
    let border = if width
        .strip_suffix("px")
        .and_then(|w| w.parse::<i32>().ok())
        .filter(|w| *w < 3)
        .is_some()
        || height
            .strip_suffix("px")
            .and_then(|h| h.parse::<i32>().ok())
            .filter(|h| *h < 3)
            .is_some()
    {
        "none"
    } else {
        "solid"
    };

    let update = |text: String| {
        if let Some(input_handler) = &cx.props.raw_oninput {
            input_handler.call(FormData {
                value: text,
                values: HashMap::new(),
            });
        }
    };

    cx.render({
        rsx! {
            div{
                width: "{width}",
                height: "{height}",
                border_style: "{border}",
                display: "flex",
                flex_direction: "row",
                align_items: "left",

                onkeydown: move |k| {
                    if matches!(k.key_code, KeyCode::LeftArrow | KeyCode::RightArrow | KeyCode::Backspace | KeyCode::Period) || k.key.chars().all(|c| c.is_numeric()) {
                        let mut text = text_ref.write();
                        cursor.write().handle_input(&*k, &mut text, max_len);
                        update(text.clone());
                    }
                    else{
                        match k.key_code {
                            KeyCode::UpArrow =>{
                                let mut text = text_ref.write();
                                *text = (text.parse::<f64>().unwrap_or(0.0) + 1.0).to_string();
                                update(text.clone());
                            }
                            KeyCode::DownArrow =>{
                                let mut text = text_ref.write();
                                *text = (text.parse::<f64>().unwrap_or(0.0) - 1.0).to_string();
                                update(text.clone());
                            }
                            _ => ()
                        }
                    }
                },

                "{text_before_first_cursor}|"

                span{
                    background_color: "rgba(100, 100, 100, 50%)",

                    "{text_highlighted}"
                }

                "{text_after_second_cursor}"

                div{
                    background_color: "rgba(255, 255, 255, 50%)",
                    color: "black",
                    Input{
                        r#type: "button",
                        onclick: move |_| {
                            let mut text = text_ref.write();
                            if let Ok(value) = text.parse::<f64>(){
                                *text = (value - 1.0).to_string();
                            }
                            update(text.clone());
                        }
                        value: "<",
                    }
                    " "
                    Input{
                        r#type: "button",
                        onclick: move |_| {
                            let mut text = text_ref.write();
                            if let Ok(value) = text.parse::<f64>(){
                                *text = (value + 1.0).to_string();
                            }
                            update(text.clone());
                        }
                        value: ">",
                    }
                }
            }
        }
    })
}
