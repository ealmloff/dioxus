use crate::{
    ElementId, VNode,
    innerlude::{ElementRef, MountId},
};

/// Whether a mount is allowed to write renderer mutations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenderMode {
    Foreground,
    Background,
}

/// Persistent render identity for one mounted `VNode`.
///
/// A mount owns the renderer ids and dynamic child bindings for an rsx block.
/// `node` is the committed view used after diffing for event dispatch, tree
/// inspection, and the next render pass.
#[derive(Debug)]
pub(crate) struct Mount {
    /// The parent used for renderer placement, anchors, and context bubbling.
    pub(crate) parent: Option<ElementRef>,

    /// The committed view used for events and mounted tree inspection.
    pub(crate) node: VNode,

    /// Suspense can keep a primary branch alive while its fallback is visible.
    /// Background mounts may update their virtual tree, but they must not write
    /// renderer mutations until they are promoted back to the foreground.
    pub(crate) mode: RenderMode,

    /// The IDs for the roots of this template, used when moving or removing
    /// roots from the renderer.
    pub(crate) root_ids: Box<[ElementId]>,

    /// The element in the renderer that each dynamic attribute is mounted to.
    pub(crate) mounted_attributes: Box<[ElementId]>,

    /// For components: the `ScopeId` the component is mounted to.
    /// For other dynamic nodes: the renderer element id each dynamic node owns.
    pub(crate) mounted_dynamic_nodes: Box<[usize]>,
}

impl Mount {
    pub(crate) fn new(
        node: VNode,
        parent: Option<ElementRef>,
        root_count: usize,
        attr_count: usize,
        dynamic_count: usize,
    ) -> Self {
        Self {
            parent,
            node,
            mode: RenderMode::Foreground,
            root_ids: vec![ElementId(0); root_count].into(),
            mounted_attributes: vec![ElementId(0); attr_count].into(),
            mounted_dynamic_nodes: vec![usize::MAX; dynamic_count].into(),
        }
    }
}

/// A retained suspense branch.
///
/// Suspense keeps the hidden primary branch alive while the fallback branch is
/// visible. The root `VNode` is still the render output we diff, but the branch
/// also records the root mount identity so the boundary state is explicitly tied
/// to retained mount ownership instead of being just a parked vnode.
#[derive(Clone, Debug)]
pub(crate) struct SuspenseBranch {
    root: VNode,
    root_mount: MountId,
}

impl SuspenseBranch {
    pub(crate) fn new(root: VNode) -> Self {
        let root_mount = root.mount.get();
        debug_assert!(
            root_mount.mounted(),
            "suspense branches must have a mounted root mount"
        );
        // Deep-clone on the way in so the stored root has its own
        // `VNodeInner`. Subsequent diffs against this branch can take per-slot
        // mounts via `claim_mount` without modifying any `Cell<MountId>`
        // shared with the parent's props or `last_rendered_node`.
        let root = root.deep_clone_preserving_mounts();
        Self { root, root_mount }
    }

    pub(crate) fn root(&self) -> VNode {
        // And one more deep-clone on the way out, so each diff pass that
        // reads the branch gets a fresh tree to consume rather than mutating
        // the stored copy across renders.
        self.root.deep_clone_preserving_mounts()
    }

    pub(crate) fn root_mount(&self) -> MountId {
        self.root_mount
    }

    pub(crate) fn into_root(self) -> VNode {
        self.root
    }
}
