use crate::driver::{ElementRect, TestElement};
use blitz_dom::{DocGuard, Node};
use dioxus_core::{ElementId, Event, VirtualDom};
use dioxus_html::{Modifiers, PlatformEventData, geometry::Coordinates};
use dioxus_native_dom::synthetic_click_event;
use std::rc::Rc;

/// A reference to DOM node managed by a [crate::DocumentTester].
///
/// This provides facilities for interacting with the node, querying its layout properties, and
/// obtaining its content.
pub struct ResolvedElement<'doc> {
    pub(crate) vdom: &'doc VirtualDom,
    pub(crate) document: DocGuard<'doc>,
    pub(crate) node_id: NodeId,
}

impl<'doc> ResolvedElement<'doc> {
    /// Dispatches a `click` event on this element.
    ///
    /// The exact location of the click is unspecified.
    ///
    /// If the element has an `onclick` handler, it will be invoked once
    /// [crate::DocumentTester::pump] is called.
    pub fn click(&self) {
        self.send_event(
            "click",
            Event::new(
                Rc::new(PlatformEventData::new(synthetic_click_event(
                    self.node_id.resolve(&self.document),
                    Modifiers::empty(),
                ))),
                true,
            ),
        );
    }

    /// Sends an event with the given `name` to this element.
    ///
    /// The event is registered with the Dioxus runtime. A subsequent call to
    /// [crate::DocumentTester::pump] causes the event handler to be invoked, if one is present.
    ///
    /// If no event handler is registered corresponding to the event `name`, then this method has no
    /// effect.
    ///
    /// This operates directly on the element, so that is is guaranteed to receive the event. This
    /// might not reflect how the element would respond in reality. For example, a click at the
    /// coordinates of a button which is behind a frost element will not reach the button. But this
    /// method behaves as though it would.
    ///
    /// The `event` parameter must contain a [PlatformEventData] with a payload corresponding to the
    /// specific event type. This method panics if the event payload has the wrong type.
    pub fn send_event(&self, name: &str, event: Event<PlatformEventData>) {
        let propagates = event.propagates();
        self.vdom.runtime().handle_event(
            name,
            Event::new(event.data, propagates),
            self.get_element_id()
                .expect("Expected element to have a Dioxus ID"),
        );
    }

    /// Returns a `String` consisting of the HTML of this element and all of its children.
    pub fn outer_html(&self) -> String {
        self.node_id.resolve(&self.document).outer_html()
    }

    /// Returns a `String` consisting of the HTML of this element's children, not including this
    /// element itself.
    pub fn inner_html(&self) -> String {
        let inner_html_parts: Vec<_> = self
            .node_id
            .resolve(&self.document)
            .children
            .iter()
            .filter_map(|child_id| {
                self.document
                    .get_node(*child_id)
                    .map(|child| child.outer_html())
            })
            .collect();
        inner_html_parts.join("")
    }

    /// Returns this element's layout box in pixels, relative to the document origin.
    pub fn bounding_rect(&self) -> ElementRect {
        let layout = &self.node_id.resolve(&self.document).final_layout;
        ElementRect {
            x: layout.location.x as f64,
            y: layout.location.y as f64,
            width: layout.content_box_width() as f64,
            height: layout.content_box_height() as f64,
        }
    }

    /// Returns the calculated [Coordinates] of the centre of this element.
    pub fn center(&self) -> Coordinates {
        TestElement::center(self)
    }

    /// Returns the calculated [Coordinates] of the upper-left corner of this element.
    pub fn upper_left(&self) -> Coordinates {
        TestElement::upper_left(self)
    }

    /// Returns the calculated [Coordinates] of the upper-right corner of this element.
    pub fn upper_right(&self) -> Coordinates {
        TestElement::upper_right(self)
    }

    /// Returns the calculated [Coordinates] of the lower-left corner of this element.
    pub fn lower_left(&self) -> Coordinates {
        TestElement::lower_left(self)
    }

    /// Returns the calculated [Coordinates] of the lower-right corner of this element.
    pub fn lower_right(&self) -> Coordinates {
        TestElement::lower_right(self)
    }

    /// Returns the calculated size of this element as a tuple (width, height) in screen pixels.
    pub fn size(&self) -> (f32, f32) {
        TestElement::size(self)
    }

    fn get_element_id(&self) -> Option<ElementId> {
        self.node_id
            .resolve(&self.document)
            .element_data()?
            .attrs
            .iter()
            .find(|attr| *attr.name.local == *"data-dioxus-id")
            .and_then(|attr| attr.value.parse::<usize>().ok())
            .map(ElementId)
    }
}

impl<'doc> std::fmt::Debug for ResolvedElement<'doc> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedElement")
            .field("node_id", &self.node_id)
            .finish()
    }
}

impl TestElement for ResolvedElement<'_> {
    fn click(&self) {
        ResolvedElement::click(self);
    }

    fn outer_html(&self) -> String {
        ResolvedElement::outer_html(self)
    }

    fn inner_html(&self) -> String {
        ResolvedElement::inner_html(self)
    }

    fn bounding_rect(&self) -> ElementRect {
        ResolvedElement::bounding_rect(self)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NodeId {
    Root,
    Node(usize),
}

impl NodeId {
    fn resolve<'doc>(self, document: &'doc DocGuard<'doc>) -> &'doc Node {
        match self {
            NodeId::Root => document.root_element(),
            NodeId::Node(node_id) => document
                .get_node(node_id)
                .expect("Element must be attached"),
        }
    }
}
