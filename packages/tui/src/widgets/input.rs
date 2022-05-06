use dioxus_core as dioxus;
use dioxus_core::prelude::fc_to_builder;
use dioxus_core::VNode;
use dioxus_core::*;
use dioxus_core_macro::*;
use dioxus_html as dioxus_elements;
use dioxus_html::on::FormData;

use crate::widgets::button::Button;
use crate::widgets::checkbox::CheckBox;
use crate::widgets::number::NumbericInput;
use crate::widgets::slider::Slider;
use crate::widgets::textbox::TextBox;

#[derive(Props)]
pub struct InputProps<'a> {
    r#type: Option<&'static str>,
    oninput: Option<EventHandler<'a, FormData>>,
    onclick: Option<EventHandler<'a, FormData>>,
    value: Option<&'a str>,
    size: Option<&'a str>,
    width: Option<&'a str>,
    height: Option<&'a str>,
    min: Option<&'a str>,
    max: Option<&'a str>,
}

#[allow(non_snake_case)]
pub fn Input<'a>(cx: Scope<'a, InputProps<'a>>) -> Element<'a> {
    cx.render(match cx.props.r#type {
        Some("checkbox") => {
            rsx! {
                CheckBox{
                    raw_oninput: cx.props.oninput.as_ref(),
                    value: cx.props.value,
                    width: cx.props.width,
                    height: cx.props.height,
                }
            }
        }
        Some("range") => {
            rsx! {
                Slider{
                    raw_oninput: cx.props.oninput.as_ref(),
                    value: cx.props.value,
                    width: cx.props.width,
                    height: cx.props.height,
                    max: cx.props.max,
                    min: cx.props.min,
                }
            }
        }
        Some("button") => {
            rsx! {
                Button{
                    raw_onclick: cx.props.onclick.as_ref(),
                    value: cx.props.value,
                    width: cx.props.width,
                    height: cx.props.height,
                }
            }
        }
        Some("number") => {
            rsx! {
                NumbericInput{
                    raw_oninput: cx.props.onclick.as_ref(),
                    value: cx.props.value,
                    size: cx.props.size,
                    width: cx.props.width,
                    height: cx.props.height,
                }
            }
        }
        _ => {
            rsx! {
                TextBox{
                    raw_oninput: cx.props.oninput.as_ref(),
                    value: cx.props.value,
                    size: cx.props.size,
                    width: cx.props.width,
                    height: cx.props.height,
                }
            }
        }
    })
}
