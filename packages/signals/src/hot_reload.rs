use std::sync::OnceLock;

use crate::{GlobalSignal, ReadableExt};
use dioxus_core::{
    Attribute, AttributeValue, DynamicNode, Runtime, TemplateNode, VNode,
    internal::{DynamicLiteralPool, DynamicValuePool, HotReloadedTemplate},
};

/// Construct an empty hot-reload template slot.
#[doc(hidden)]
pub fn empty_hot_reload_template() -> Option<HotReloadedTemplate> {
    None
}

/// Storage for the original template associated with an rsx callsite.
#[doc(hidden)]
pub type HotReloadedTemplateLock = OnceLock<HotReloadedTemplate>;

/// Signal slot used by hot reload to replace an rsx callsite template.
#[doc(hidden)]
pub type HotReloadTemplateSignal = GlobalSignal<Option<HotReloadedTemplate>>;

/// Render a VNode through the Dioxus hot-reload template path.
#[doc(hidden)]
pub fn render_hot_reload_template(
    original: &'static HotReloadedTemplateLock,
    original_factory: fn() -> HotReloadedTemplate,
    template: &'static HotReloadTemplateSignal,
    dynamic_nodes: Box<[DynamicNode]>,
    dynamic_attributes: Box<[Box<[Attribute]>]>,
    dynamic_text: Box<[String]>,
) -> VNode {
    let mut dynamic_value_pool = DynamicValuePool::new_boxed(
        dynamic_nodes,
        dynamic_attributes,
        DynamicLiteralPool::new_boxed(dynamic_text),
    );

    if let Some(template_read) = Runtime::try_current().map(|_| template.read())
        && let Some(template_read) = template_read.as_ref()
    {
        return dynamic_value_pool.render_with(template_read);
    }

    let original = original.get_or_init(original_factory);
    dynamic_value_pool.render_with(original)
}

/// Render a VNode through the hot-reload path when the template only has dynamic attributes.
#[doc(hidden)]
pub fn render_hot_reload_template_with_dynamic_attrs(
    original: &'static HotReloadedTemplateLock,
    template: &'static HotReloadTemplateSignal,
    roots: &'static [TemplateNode],
    dynamic_attributes: Box<[Box<[Attribute]>]>,
    dynamic_text: Box<[String]>,
) -> VNode {
    let dynamic_attribute_count = dynamic_attributes.len();
    let dynamic_nodes: Box<[DynamicNode]> = Box::new([]);
    let mut dynamic_value_pool = DynamicValuePool::new_boxed(
        dynamic_nodes,
        dynamic_attributes,
        DynamicLiteralPool::new_boxed(dynamic_text),
    );

    if let Some(template_read) = Runtime::try_current().map(|_| template.read())
        && let Some(template_read) = template_read.as_ref()
    {
        return dynamic_value_pool.render_with(template_read);
    }

    let original = original.get_or_init(|| {
        HotReloadedTemplate::new_with_dynamic_mapping(
            None,
            0,
            dynamic_attribute_count,
            Vec::new(),
            roots,
        )
    });
    dynamic_value_pool.render_with(original)
}

/// Render a VNode through the hot-reload path for the common SVG size/color/stroke attr set.
#[doc(hidden)]
pub fn render_hot_reload_template_with_svg_stroke_attrs(
    original: &'static HotReloadedTemplateLock,
    template: &'static HotReloadTemplateSignal,
    roots: &'static [TemplateNode],
    width: String,
    height: String,
    stroke: String,
    stroke_width: String,
    stroke_linecap: String,
    stroke_linejoin: String,
    class: AttributeValue,
) -> VNode {
    let dynamic_attributes = Attribute::svg_size_color_stroke_attr_slots(
        AttributeValue::Text(width.clone()),
        AttributeValue::Text(height.clone()),
        AttributeValue::Text(stroke.clone()),
        AttributeValue::Text(stroke_width.clone()),
        AttributeValue::Text(stroke_linecap.clone()),
        AttributeValue::Text(stroke_linejoin.clone()),
        class,
    );

    render_hot_reload_template_with_dynamic_attrs(
        original,
        template,
        roots,
        dynamic_attributes,
        Box::new([
            width,
            height,
            stroke,
            stroke_width,
            stroke_linecap,
            stroke_linejoin,
        ]),
    )
}
