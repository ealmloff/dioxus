use crate::innerlude::ScopeOrder;
use crate::{ScopeId, virtual_dom::VirtualDom};

/// An Element's unique identifier.
///
/// `ElementId` is a `usize` that is unique across the entire VirtualDom - but
/// not unique across time. If a component is unmounted, then the `ElementId`
/// may be reused for a new component.
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ElementId(pub usize);

impl ElementId {
    /// The root element of the VirtualDom.
    pub const ROOT: Self = Self(0);
}

/// A mounted mount's unique identifier.
///
/// `MountId` is a `usize` that is unique across the current `VirtualDom` - but not unique across time. If a mount is
/// unmounted, then the `MountId` may be reused for a new mount.
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MountId(pub(crate) usize);

impl Default for MountId {
    fn default() -> Self {
        Self::PLACEHOLDER
    }
}

impl MountId {
    pub(crate) const PLACEHOLDER: Self = Self(usize::MAX);

    #[allow(unused)]
    pub(crate) fn mounted(self) -> bool {
        self != Self::PLACEHOLDER
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ElementRef {
    // the pathway of the real element inside the template
    pub(crate) path: ElementPath,

    // The actual element
    pub(crate) mount: MountId,
}

#[derive(Clone, Copy, Debug)]
pub struct ElementPath {
    pub(crate) path: &'static [u8],
}

impl VirtualDom {
    /// Number of template roots this `mount` was created with.
    /// Anchor lookups that walk a view's `template.roots()` may iterate
    /// beyond what the mount actually has — e.g. when the view was a clone
    /// whose template grew between renders — and the underlying
    /// `Mount::root_ids` would panic on out-of-range indexing.
    pub(crate) fn mounted_root_count(&self, mount: MountId) -> usize {
        debug_assert!(
            mount.mounted(),
            "mounted_root_count requires a live MountId"
        );
        self.runtime
            .mounts
            .borrow()
            .get(mount.0)
            .map(|mount| mount.root_ids.len())
            .unwrap_or(0)
    }

    /// Number of dynamic-node slots this `mount` was created with.
    /// Same guard rail as [`Self::mounted_root_count`], but for
    /// `Mount::mounted_dynamic_nodes`.
    pub(crate) fn mounted_dyn_node_count(&self, mount: MountId) -> usize {
        debug_assert!(
            mount.mounted(),
            "mounted_dyn_node_count requires a live MountId"
        );
        self.runtime
            .mounts
            .borrow()
            .get(mount.0)
            .map(|mount| mount.mounted_dynamic_nodes.len())
            .unwrap_or(0)
    }

    pub(crate) fn next_element(&mut self) -> ElementId {
        let mut elements = self.runtime.elements.borrow_mut();
        ElementId(elements.insert(None))
    }

    pub(crate) fn try_reclaim(&mut self, el: ElementId) -> bool {
        // Callers always pre-filter `ElementId::default()` (ROOT) and the
        // PLACEHOLDER sentinel, so we never see id 0 or usize::MAX here.
        debug_assert!(
            el.0 != 0 && el.0 != usize::MAX,
            "try_reclaim should never see ROOT or PLACEHOLDER ids",
        );

        let mut elements = self.runtime.elements.borrow_mut();
        elements.try_remove(el.0).is_some()
    }

    pub(crate) fn element_exists(&self, el: ElementId) -> bool {
        // Callers in diff/anchor.rs and diff/node.rs always pre-filter
        // `ElementId::default()` (ROOT) before calling, so we never see id 0.
        debug_assert!(el.0 != 0, "element_exists should never see ROOT");

        self.runtime.elements.borrow().get(el.0).is_some()
    }

    // Drop a scope without dropping its children
    //
    // Note: This will not remove any ids from the arena
    pub(crate) fn drop_scope(&mut self, id: ScopeId) {
        let stale_dirty_scopes: Vec<_> = self
            .dirty_scopes
            .iter()
            .filter_map(|order| {
                (order.id == id || self.runtime.is_descendant_of(order.id, id)).then_some(*order)
            })
            .collect();

        let stale_dirty_tasks: Vec<_> = self
            .runtime
            .dirty_tasks
            .borrow()
            .iter()
            .filter_map(|tasks| {
                (tasks.order.id == id || self.runtime.is_descendant_of(tasks.order.id, id))
                    .then_some(tasks.order)
            })
            .collect();

        let stale_pending_effects: Vec<_> = self
            .runtime
            .pending_effects
            .borrow()
            .iter()
            .filter_map(|effect| {
                (effect.order.id == id || self.runtime.is_descendant_of(effect.order.id, id))
                    .then_some(effect.order)
            })
            .collect();

        let scope = self.scopes.remove(id.0);
        let context = scope.state();
        let height = context.height;

        self.dirty_scopes.remove(&ScopeOrder::new(height, id));
        for order in stale_dirty_scopes {
            self.dirty_scopes.remove(&order);
        }
        {
            let mut dirty_tasks = self.runtime.dirty_tasks.borrow_mut();
            for order in stale_dirty_tasks {
                dirty_tasks.remove(&order);
            }
        }
        {
            let mut pending_effects = self.runtime.pending_effects.borrow_mut();
            for order in stale_pending_effects {
                pending_effects.remove(&order);
            }
        }

        // If this scope was a suspense boundary, remove it from the resolved scopes
        self.resolved_scopes.retain(|s| s != &id);
    }
}

impl ElementPath {
    pub(crate) fn is_descendant(&self, small: &[u8]) -> bool {
        small.len() <= self.path.len() && small == &self.path[..small.len()]
    }
}

#[test]
fn is_descendant() {
    let event_path = ElementPath {
        path: &[1, 2, 3, 4, 5],
    };

    assert!(event_path.is_descendant(&[1, 2, 3, 4, 5]));
    assert!(event_path.is_descendant(&[1, 2, 3, 4]));
    assert!(event_path.is_descendant(&[1, 2, 3]));
    assert!(event_path.is_descendant(&[1, 2]));
    assert!(event_path.is_descendant(&[1]));

    assert!(!event_path.is_descendant(&[1, 2, 3, 4, 5, 6]));
    assert!(!event_path.is_descendant(&[2, 3, 4]));
}

impl PartialEq<&[u8]> for ElementPath {
    fn eq(&self, other: &&[u8]) -> bool {
        self.path.eq(*other)
    }
}
