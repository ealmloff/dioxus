use crate::driver::{BoundingBox, Driver};
use dioxus_html::geometry::{ClientPoint, Coordinates, ElementPoint, PagePoint, ScreenPoint};

/// A reference to a node in the DOM managed by a [crate::DocumentTester].
///
/// Holds an opaque [Driver::NodeHandle] plus a shared borrow of the [Driver]. All operations
/// route through the driver, so the same element type works against any backend.
///
/// Because the driver borrow is shared, multiple [ResolvedElement]s can coexist for the same
/// tester (e.g. inside collection matchers).
pub struct ResolvedElement<'a, D: Driver> {
    pub(crate) handle: D::NodeHandle,
    pub(crate) driver: &'a D,
}

impl<'a, D: Driver> ResolvedElement<'a, D> {
    /// Returns the backend-specific handle to this node.
    pub fn handle(&self) -> D::NodeHandle {
        self.handle.clone()
    }

    /// Dispatches a `click` event on this element.
    ///
    /// If the element has an `onclick` handler, it will be invoked once
    /// [crate::DocumentTester::pump] is called.
    pub async fn click(&self) {
        self.driver.click(self.handle.clone()).await
    }

    /// Returns the inner HTML of this element.
    pub async fn inner_html(&self) -> String {
        self.driver.inner_html(self.handle.clone()).await
    }

    /// Returns the outer HTML of this element.
    pub async fn outer_html(&self) -> String {
        self.driver.outer_html(self.handle.clone()).await
    }

    /// Returns the layout-resolved bounding box of this element in CSS pixels.
    pub async fn bounding_box(&self) -> BoundingBox {
        self.driver.bounding_box(self.handle.clone()).await
    }

    /// Returns the (width, height) of this element in CSS pixels.
    pub async fn size(&self) -> (f64, f64) {
        self.bounding_box().await.size()
    }

    /// Returns the [Coordinates] of the centre of this element.
    pub async fn center(&self) -> Coordinates {
        point_coordinates(self.bounding_box().await.center())
    }

    /// Returns the [Coordinates] of the upper-left corner of this element.
    pub async fn upper_left(&self) -> Coordinates {
        point_coordinates(self.bounding_box().await.upper_left())
    }

    /// Returns the [Coordinates] of the upper-right corner of this element.
    pub async fn upper_right(&self) -> Coordinates {
        point_coordinates(self.bounding_box().await.upper_right())
    }

    /// Returns the [Coordinates] of the lower-left corner of this element.
    pub async fn lower_left(&self) -> Coordinates {
        point_coordinates(self.bounding_box().await.lower_left())
    }

    /// Returns the [Coordinates] of the lower-right corner of this element.
    pub async fn lower_right(&self) -> Coordinates {
        point_coordinates(self.bounding_box().await.lower_right())
    }
}

impl<D: Driver> std::fmt::Debug for ResolvedElement<'_, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedElement")
            .field("handle", &self.handle)
            .finish()
    }
}

/// Lifts a `(x, y)` document-relative point into the four typed coordinate spaces.
///
/// The in-process backend renders without a window or scroll offset, so screen, client, page and
/// element-relative coordinates all coincide. Remote drivers are free to differentiate later.
fn point_coordinates((x, y): (f64, f64)) -> Coordinates {
    Coordinates::new(
        ScreenPoint::new(x, y),
        ClientPoint::new(x, y),
        ElementPoint::new(x, y),
        PagePoint::new(x, y),
    )
}
