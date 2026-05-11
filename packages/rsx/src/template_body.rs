//! I'm so sorry this is so complicated. Here's my best to simplify and explain it:
//!
//! The `Callbody` is the contents of the rsx! macro - this contains all the information about every
//! node that rsx! directly knows about. For loops, if statements, etc.
//!
//! However, there are multiple *templates* inside a callbody - due to how core clones templates and
//! just generally rationalize the concept of a template, nested bodies like for loops and if statements
//! and component children are all templates, contained within the same Callbody.
//!
//! This gets confusing fast since there's lots of IDs bouncing around.
//!
//! The IDs at play:
//! - The id of the template itself so we can find it and apply it to the dom.
//!   This is challenging since all calls to file/line/col/id are relative to the macro invocation,
//!   so they will have to share the same base ID and we need to give each template a new ID.
//!   The id of the template will be something like file!():line!():col!():ID where ID increases for
//!   each nested template.
//!
//! - The IDs of dynamic nodes relative to the template they live in. This is somewhat easy to track
//!   but needs to happen on a per-template basis.
//!
//! - The IDs of formatted strings in debug mode only. Any formatted segments like "{x:?}" get pulled out
//!   into a pool so we can move them around during hot reloading on a per-template basis.
//!
//! - The IDs of component property literals in debug mode only. Any component property literals like
//!   1234 get pulled into the pool so we can hot reload them with the context of the literal pool.
//!
//! We solve this by parsing the structure completely and then doing a second pass that fills in IDs
//! by walking the structure.
//!
//! This means you can't query the ID of any node "in a vacuum" - these are assigned once - but at
//! least they're stable enough for the purposes of hotreloading
//!
//! ```text
//! rsx! {
//!     div {
//!         class: "hello",
//!         id: "node-{node_id}",         <--- {node_id} has the formatted segment id 0 in the literal pool
//!         ..props,                      <--- spreads are not reloadable
//!
//!         "Hello, world!"               <--- not tracked but reloadable in the template since it's just a string
//!
//!         for item in 0..10 {           <--- both 0 and 10 are technically reloadable, but we don't hot reload them today...
//!             div { "cool-{item}" }     <--- {item} has the formatted segment id 1 in the literal pool
//!         }
//!
//!         Link {
//!             to: "/home",              <--- hotreloadable since its a component prop literal (with component literal id 0)
//!             class: "link {is_ready}", <--- {is_ready} has the formatted segment id 2 in the literal pool and the property has the component literal id 1
//!             "Home"                    <--- hotreloadable since its a component child (via template)
//!         }
//!     }
//! }
//! ```

use self::location::DynIdx;
use crate::innerlude::Attribute;
use crate::*;
use proc_macro2::{Span, TokenStream as TokenStream2};
use proc_macro2_diagnostics::SpanDiagnosticExt;
use syn::parse_quote;

type NodePath = Vec<u8>;
type AttributePath = Vec<u8>;

fn common_svg_root_dynamic_attributes(attrs: &[&Attribute]) -> Option<TokenStream2> {
    const COMMON_SVG_ATTRS: [&str; 7] = [
        "width",
        "height",
        "stroke",
        "stroke_width",
        "stroke_linecap",
        "stroke_linejoin",
        "class",
    ];

    if attrs.len() != COMMON_SVG_ATTRS.len() {
        return None;
    }

    let values = attrs
        .iter()
        .zip(COMMON_SVG_ATTRS)
        .map(|(attr, expected)| attr.rendered_as_common_svg_root_attr_value(expected))
        .collect::<Option<Vec<_>>>()?;

    Some(quote! {
        dioxus_core::Attribute::svg_size_color_stroke_attr_slots(#( #values ),*)
    })
}

fn common_svg_root_hot_reload_attributes(attrs: &[&Attribute]) -> Option<TokenStream2> {
    const COMMON_SVG_TEXT_ATTRS: [&str; 6] = [
        "width",
        "height",
        "stroke",
        "stroke_width",
        "stroke_linecap",
        "stroke_linejoin",
    ];

    if attrs.len() != COMMON_SVG_TEXT_ATTRS.len() + 1 {
        return None;
    }

    let text_values = attrs
        .iter()
        .take(COMMON_SVG_TEXT_ATTRS.len())
        .zip(COMMON_SVG_TEXT_ATTRS)
        .map(|(attr, expected)| attr.rendered_as_common_svg_root_text_value(expected))
        .collect::<Option<Vec<_>>>()?;
    let class_value =
        attrs[COMMON_SVG_TEXT_ATTRS.len()].rendered_as_common_svg_root_attr_value("class")?;

    Some(quote! {
        #( #text_values, )*
        #class_value
    })
}

fn dynamic_attributes_tokens(dynamic_attributes: &[&Attribute]) -> TokenStream2 {
    if let Some(common_svg_attrs) = common_svg_root_dynamic_attributes(dynamic_attributes) {
        common_svg_attrs
    } else if dynamic_attributes.iter().all(|attr| !attr.is_spread()) {
        let dyn_attr_value_slots = dynamic_attributes
            .iter()
            .map(|attr| attr.rendered_as_dynamic_attr_value_slot())
            .collect::<Option<Vec<_>>>();

        if let Some(dyn_attr_value_slots) = dyn_attr_value_slots {
            quote! { dioxus_core::Attribute::single_attr_value_slots([ #( #dyn_attr_value_slots ),* ]) }
        } else {
            let dyn_attr_printer: Vec<_> = dynamic_attributes
                .iter()
                .map(|attr| {
                    attr.rendered_as_dynamic_single_attr()
                        .expect("spread attributes should be handled by the boxed attribute path")
                })
                .collect();
            quote! { dioxus_core::Attribute::single_attr_slots([ #( #dyn_attr_printer ),* ]) }
        }
    } else {
        let dyn_attr_printer: Vec<_> = dynamic_attributes
            .iter()
            .map(|attr| attr.rendered_as_dynamic_attr())
            .collect();
        quote! { Box::new([ #( #dyn_attr_printer ),* ]) }
    }
}

/// A set of nodes in a template position
///
/// this could be:
/// - The root of a callbody
/// - The children of a component
/// - The children of a for loop
/// - The children of an if chain
///
/// The TemplateBody when needs to be parsed into a surrounding `Body` to be correctly re-indexed
/// By default every body has a `0` default index
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct TemplateBody {
    pub roots: Vec<BodyNode>,
    pub template_idx: DynIdx,
    pub node_paths: Vec<NodePath>,
    pub attr_paths: Vec<(AttributePath, usize)>,
    pub dynamic_text_segments: Vec<FormattedSegment>,
    pub diagnostics: Diagnostics,
}

impl Parse for TemplateBody {
    /// Parse the nodes of the callbody as `Body`.
    fn parse(input: ParseStream) -> Result<Self> {
        let children = RsxBlock::parse_children(input)?;
        let mut myself = Self::new(children.children);
        myself
            .diagnostics
            .extend(children.diagnostics.into_diagnostics());

        Ok(myself)
    }
}

/// Our ToTokens impl here just defers to rendering a template out like any other `Body`.
/// This is because the parsing phase filled in all the additional metadata we need
impl ToTokens for TemplateBody {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        // First normalize the template body for rendering
        let node = self.normalized();

        // If we have an implicit key, then we need to write its tokens
        let implicit_key = node.implicit_key();
        let has_key = implicit_key.is_some();
        let key_tokens = match implicit_key {
            Some(tok) => quote! { Some( #tok.to_string() ) },
            None => quote! { None },
        };

        let key_warnings = self.check_for_duplicate_keys();

        let roots = node.quote_roots();

        // For printing dynamic nodes, we rely on the ToTokens impl
        // Elements have a weird ToTokens - they actually are the entrypoint for Template creation
        let dynamic_nodes: Vec<_> = node.dynamic_nodes().collect();

        // We could add a ToTokens for Attribute but since we use that for both components and elements
        // They actually need to be different, so we just localize that here
        let dynamic_attributes: Vec<_> = node.dynamic_attributes().collect();

        let diagnostics = &node.diagnostics;

        if std::env::var_os("DIOXUS_HOT_RELOAD").is_some() {
            let index = node.template_idx.get();
            let has_component_literals = node.literal_component_properties().next().is_some();

            if has_component_literals {
                let dynamic_text = node.dynamic_text_segments.iter();
                let dynamic_attributes_tokens = dynamic_attributes_tokens(&dynamic_attributes);
                let hot_reload_mapping = node.hot_reload_mapping();

                tokens.append_all(quote! {
                    dioxus_core::Element::Ok({
                        #diagnostics

                        #key_warnings

                        #[doc(hidden)]
                        static __TEMPLATE_ROOTS: &[dioxus_core::TemplateNode] = &[ #( #roots ),* ];

                        fn __original_template() -> &'static dioxus_core::internal::HotReloadedTemplate {
                            static __ORIGINAL_TEMPLATE: dioxus_signals::HotReloadedTemplateLock = dioxus_signals::HotReloadedTemplateLock::new();
                            __ORIGINAL_TEMPLATE.get_or_init(|| #hot_reload_mapping)
                        }

                        let __template_read = {
                            use dioxus_signals::ReadableExt;

                            static __TEMPLATE: dioxus_signals::HotReloadTemplateSignal = dioxus_signals::HotReloadTemplateSignal::with_location(
                                dioxus_signals::empty_hot_reload_template,
                                file!(),
                                line!(),
                                column!(),
                                #index
                            );

                            dioxus_core::Runtime::try_current().map(|_| __TEMPLATE.read())
                        };

                        let __template_read = match __template_read.as_ref().map(|__template_read| __template_read.as_ref()) {
                            Some(Some(__template_read)) => &__template_read,
                            _ => __original_template(),
                        };

                        let mut __dynamic_literal_pool = dioxus_core::internal::DynamicLiteralPool::new(
                            vec![ #( #dynamic_text ),* ],
                        );

                        let __dynamic_nodes: Box<[dioxus_core::DynamicNode]> = Box::new([ #( #dynamic_nodes ),* ]);
                        let __dynamic_attributes = #dynamic_attributes_tokens;

                        {
                            let mut __dynamic_value_pool = dioxus_core::internal::DynamicValuePool::new_boxed(
                                __dynamic_nodes,
                                __dynamic_attributes,
                                __dynamic_literal_pool
                            );
                            __dynamic_value_pool.render_with(__template_read)
                        }
                    })
                });
            } else {
                let dynamic_text = node.dynamic_text_segments.iter();
                let common_svg_hot_attrs =
                    common_svg_root_hot_reload_attributes(&dynamic_attributes);

                if !has_key && dynamic_nodes.is_empty() {
                    if let Some(common_svg_hot_attrs) = common_svg_hot_attrs {
                        tokens.append_all(quote! {
                            dioxus_core::Element::Ok({
                                #diagnostics

                                #key_warnings

                                #[doc(hidden)]
                                static __TEMPLATE_ROOTS: &[dioxus_core::TemplateNode] = &[ #( #roots ),* ];

                                {
                                    static __ORIGINAL_TEMPLATE: dioxus_signals::HotReloadedTemplateLock = dioxus_signals::HotReloadedTemplateLock::new();
                                    static __TEMPLATE: dioxus_signals::HotReloadTemplateSignal = dioxus_signals::HotReloadTemplateSignal::with_location(
                                        dioxus_signals::empty_hot_reload_template,
                                        file!(),
                                        line!(),
                                        column!(),
                                        #index
                                    );

                                    dioxus_signals::render_hot_reload_template_with_svg_stroke_attrs(
                                        &__ORIGINAL_TEMPLATE,
                                        &__TEMPLATE,
                                        __TEMPLATE_ROOTS,
                                        #common_svg_hot_attrs,
                                    )
                                }
                            })
                        });
                    } else {
                        let dynamic_attributes_tokens =
                            dynamic_attributes_tokens(&dynamic_attributes);
                        tokens.append_all(quote! {
                            dioxus_core::Element::Ok({
                                #diagnostics

                                #key_warnings

                                #[doc(hidden)]
                                static __TEMPLATE_ROOTS: &[dioxus_core::TemplateNode] = &[ #( #roots ),* ];

                                let __dynamic_attributes = #dynamic_attributes_tokens;

                                {
                                    static __ORIGINAL_TEMPLATE: dioxus_signals::HotReloadedTemplateLock = dioxus_signals::HotReloadedTemplateLock::new();
                                    static __TEMPLATE: dioxus_signals::HotReloadTemplateSignal = dioxus_signals::HotReloadTemplateSignal::with_location(
                                        dioxus_signals::empty_hot_reload_template,
                                        file!(),
                                        line!(),
                                        column!(),
                                        #index
                                    );

                                    dioxus_signals::render_hot_reload_template_with_dynamic_attrs(
                                        &__ORIGINAL_TEMPLATE,
                                        &__TEMPLATE,
                                        __TEMPLATE_ROOTS,
                                        __dynamic_attributes,
                                        Box::new([ #( #dynamic_text ),* ]),
                                    )
                                }
                            })
                        });
                    }
                } else {
                    let dynamic_attributes_tokens = dynamic_attributes_tokens(&dynamic_attributes);
                    let hot_reload_mapping = node.hot_reload_mapping();
                    tokens.append_all(quote! {
                        dioxus_core::Element::Ok({
                            #diagnostics

                            #key_warnings

                            #[doc(hidden)]
                            static __TEMPLATE_ROOTS: &[dioxus_core::TemplateNode] = &[ #( #roots ),* ];

                            let __dynamic_nodes: Box<[dioxus_core::DynamicNode]> = Box::new([ #( #dynamic_nodes ),* ]);
                            let __dynamic_attributes = #dynamic_attributes_tokens;

                            {
                                fn __original_template_factory() -> dioxus_core::internal::HotReloadedTemplate {
                                    #hot_reload_mapping
                                }

                                static __ORIGINAL_TEMPLATE: dioxus_signals::HotReloadedTemplateLock = dioxus_signals::HotReloadedTemplateLock::new();
                                static __TEMPLATE: dioxus_signals::HotReloadTemplateSignal = dioxus_signals::HotReloadTemplateSignal::with_location(
                                    dioxus_signals::empty_hot_reload_template,
                                    file!(),
                                    line!(),
                                    column!(),
                                    #index
                                );

                                dioxus_signals::render_hot_reload_template(
                                    &__ORIGINAL_TEMPLATE,
                                    __original_template_factory,
                                    &__TEMPLATE,
                                    __dynamic_nodes,
                                    __dynamic_attributes,
                                    Box::new([ #( #dynamic_text ),* ]),
                                )
                            }
                        })
                    });
                }
            }
        } else {
            // Print paths is easy - just print the paths
            let node_paths = node.node_paths.iter().map(|it| quote!(&[#(#it),*]));
            let attr_paths = node.attr_paths.iter().map(|(it, _)| quote!(&[#(#it),*]));
            let template = quote! {
                #[doc(hidden)] // vscode please stop showing these in symbol search
                static ___TEMPLATE: dioxus_core::Template = dioxus_core::Template::new(
                    __TEMPLATE_ROOTS,
                    &[ #( #node_paths ),* ],
                    &[ #( #attr_paths ),* ],
                );
            };
            let vnode = quote! {
                // NOTE: Allocating a temporary is important to make reads within rsx drop before the value is returned
                #[allow(clippy::let_and_return)]
                let __vnodes = dioxus_core::VNode::new(
                    __key,
                    ___TEMPLATE,
                    __dynamic_nodes,
                    __dynamic_attributes,
                );
                __vnodes
            };
            let direct_vnode = if !has_key && dynamic_nodes.is_empty() {
                quote! {
                    // NOTE: Allocating a temporary is important to make reads within rsx drop before the value is returned
                    #[allow(clippy::let_and_return)]
                    let __vnodes = dioxus_core::VNode::new_with_dynamic_attrs(
                        ___TEMPLATE,
                        __dynamic_attributes,
                    );
                    __vnodes
                }
            } else {
                vnode.clone()
            };
            let direct_dynamic_inputs = if !has_key && dynamic_nodes.is_empty() {
                quote! {}
            } else {
                quote! {
                    // The key needs to be created before the dynamic nodes as it might depend on a borrowed value which gets moved into the dynamic nodes
                    let __key = #key_tokens;
                    let __dynamic_nodes: Box<[dioxus_core::DynamicNode]> = Box::new([ #( #dynamic_nodes ),* ]);
                }
            };
            let dynamic_attributes_tokens = dynamic_attributes_tokens(&dynamic_attributes);
            tokens.append_all(quote! {
                dioxus_core::Element::Ok({
                    #diagnostics

                    #key_warnings

                    #direct_dynamic_inputs
                    let __dynamic_attributes = #dynamic_attributes_tokens;
                    #[doc(hidden)]
                    static __TEMPLATE_ROOTS: &[dioxus_core::TemplateNode] = &[ #( #roots ),* ];
                    #template
                    #direct_vnode
                })
            });
        }
    }
}

impl TemplateBody {
    /// Create a new TemplateBody from a set of nodes
    ///
    /// This will fill in all the necessary path information for the nodes in the template and will
    /// overwrite data like dynamic indexes.
    pub fn new(nodes: Vec<BodyNode>) -> Self {
        let mut body = Self {
            roots: vec![],
            template_idx: DynIdx::default(),
            node_paths: Vec::new(),
            attr_paths: Vec::new(),
            dynamic_text_segments: Vec::new(),
            diagnostics: Diagnostics::new(),
        };

        // Assign paths to all nodes in the template
        body.assign_paths_inner(&nodes);

        // And then save the roots
        body.roots = nodes;

        // Finally, validate the key
        body.validate_key();

        body
    }

    /// Normalize the Template body for rendering. If the body is completely empty, insert a placeholder node
    pub fn normalized(&self) -> Self {
        // If the nodes are completely empty, insert a placeholder node
        // Core expects at least one node in the template to make it easier to replace
        if self.is_empty() {
            // Create an empty template body with a placeholder and diagnostics + the template index from the original
            let empty = Self::new(vec![BodyNode::RawExpr(parse_quote! {()})]);
            let default = Self {
                diagnostics: self.diagnostics.clone(),
                template_idx: self.template_idx.clone(),
                ..empty
            };
            return default;
        }
        self.clone()
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    pub fn implicit_key(&self) -> Option<&AttributeValue> {
        self.roots.first().and_then(BodyNode::key)
    }

    /// Ensure only one key and that the key is not a static str
    ///
    /// todo: we want to allow arbitrary exprs for keys provided they impl hash / eq
    fn validate_key(&mut self) {
        let key = self.implicit_key();

        if let Some(attr) = key {
            let diagnostic = match &attr {
                AttributeValue::AttrLiteral(ifmt) => {
                    if ifmt.is_static() {
                        ifmt.span().error("Key must not be a static string. Make sure to use a formatted string like `key: \"{value}\"")
                    } else {
                        return;
                    }
                }
                _ => attr
                    .span()
                    .error("Key must be in the form of a formatted string like `key: \"{value}\""),
            };

            self.diagnostics.push(diagnostic);
        }
    }

    fn check_for_duplicate_keys(&self) -> TokenStream2 {
        let mut warnings = TokenStream2::new();

        // Make sure there are not multiple keys or keys on nodes other than the first in the block
        for root in self.roots.iter().skip(1) {
            if let Some(key) = root.key() {
                warnings.extend(new_diagnostics::warning_diagnostic(
                    key.span(),
                    "Keys are only allowed on the first node in the block.",
                ));
            }
        }

        warnings
    }

    pub fn get_dyn_node(&self, path: &[u8]) -> &BodyNode {
        let mut node = self.roots.get(path[0] as usize).unwrap();
        for idx in path.iter().skip(1) {
            node = node.element_children().get(*idx as usize).unwrap();
        }
        node
    }

    pub fn get_dyn_attr(&self, path: &AttributePath, idx: usize) -> &Attribute {
        match self.get_dyn_node(path) {
            BodyNode::Element(el) => &el.merged_attributes[idx],
            _ => unreachable!(),
        }
    }

    pub fn dynamic_attributes(&self) -> impl DoubleEndedIterator<Item = &Attribute> {
        self.attr_paths
            .iter()
            .map(|(path, idx)| self.get_dyn_attr(path, *idx))
    }

    pub fn dynamic_nodes(&self) -> impl DoubleEndedIterator<Item = &BodyNode> {
        self.node_paths.iter().map(|path| self.get_dyn_node(path))
    }

    fn quote_roots(&self) -> impl Iterator<Item = TokenStream2> + '_ {
        self.roots.iter().map(|node| match node {
            BodyNode::Element(el) => quote! { #el },
            BodyNode::Text(text) if text.is_static() => {
                let text = text.input.to_static().unwrap();
                quote! { dioxus_core::TemplateNode::text(#text) }
            }
            _ => {
                let id = node.get_dyn_idx();
                quote! { dioxus_core::TemplateNode::dynamic(#id) }
            }
        })
    }

    /// Iterate through the literal component properties of this rsx call in depth-first order
    pub fn literal_component_properties(&self) -> impl Iterator<Item = &HotLiteral> + '_ {
        self.dynamic_nodes()
            .filter_map(|node| {
                if let BodyNode::Component(component) = node {
                    Some(component)
                } else {
                    None
                }
            })
            .flat_map(|component| {
                component.component_props().filter_map(|field| {
                    if let AttributeValue::AttrLiteral(literal) = &field.value {
                        Some(literal)
                    } else {
                        None
                    }
                })
            })
    }

    fn hot_reload_mapping(&self) -> TokenStream2 {
        let key = if let Some(AttributeValue::AttrLiteral(HotLiteral::Fmted(key))) =
            self.implicit_key()
        {
            quote! { Some(#key) }
        } else {
            quote! { None }
        };
        let dynamic_node_count = self.node_paths.len();
        let dynamic_attribute_count = self.attr_paths.len();
        let component_values = self
            .literal_component_properties()
            .map(|literal| literal.quote_as_hot_reload_literal())
            .collect::<Vec<_>>();
        let component_values = if component_values.is_empty() {
            quote! { Vec::new() }
        } else {
            quote! { vec![ #( #component_values ),* ] }
        };
        quote! {
            dioxus_core::internal::HotReloadedTemplate::new_with_dynamic_mapping(
                #key,
                #dynamic_node_count,
                #dynamic_attribute_count,
                #component_values,
                __TEMPLATE_ROOTS,
            )
        }
    }

    /// Get the span of the first root of this template
    pub(crate) fn first_root_span(&self) -> Span {
        match self.roots.first() {
            Some(root) => root.span(),
            _ => Span::call_site(),
        }
    }
}
