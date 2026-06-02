use super::{Synthetic, WebEventExt};
use dioxus_html::{HasSelectionData, SelectionDirection, TextSelection};
use wasm_bindgen::JsCast;

impl HasSelectionData for Synthetic<web_sys_x::Event> {
    fn selection(&self) -> Option<TextSelection> {
        let target = self.event.target()?;

        if let Some(input) = target.dyn_ref::<web_sys_x::HtmlInputElement>() {
            let start = input.selection_start().ok()?? as usize;
            let end = input.selection_end().ok()?? as usize;
            let direction = input.selection_direction().ok().flatten();
            return Some(TextSelection::new(
                start..end,
                parse_selection_direction(direction.as_deref()),
            ));
        }

        if let Some(textarea) = target.dyn_ref::<web_sys_x::HtmlTextAreaElement>() {
            let start = textarea.selection_start().ok()?? as usize;
            let end = textarea.selection_end().ok()?? as usize;
            let direction = textarea.selection_direction().ok().flatten();
            return Some(TextSelection::new(
                start..end,
                parse_selection_direction(direction.as_deref()),
            ));
        }

        None
    }

    fn as_any(&self) -> &dyn std::any::Any {
        &self.event
    }
}

fn parse_selection_direction(direction: Option<&str>) -> SelectionDirection {
    match direction {
        Some("forward") => SelectionDirection::Forward,
        Some("backward") => SelectionDirection::Backward,
        _ => SelectionDirection::None,
    }
}

impl WebEventExt for dioxus_html::SelectionData {
    type WebEvent = web_sys_x::Event;

    #[inline(always)]
    fn try_as_web_event(&self) -> Option<Self::WebEvent> {
        self.downcast::<web_sys_x::Event>().cloned()
    }
}
