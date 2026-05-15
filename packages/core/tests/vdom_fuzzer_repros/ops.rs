use crate::model::*;
use rand::{
    Rng,
    distr::{Distribution, StandardUniform},
};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

// ---------- Random operation generation -------------------------------------------------------

fn random_namespace<R: Rng + ?Sized>(rng: &mut R) -> Option<u8> {
    (rng.random_range(0..3) == 0).then(|| rng.random())
}

fn random_key<R: Rng + ?Sized>(rng: &mut R) -> Option<u8> {
    (rng.random_range(0..3) == 0).then(|| rng.random())
}

fn random_fragment_key_mode<R: Rng + ?Sized>(rng: &mut R) -> FragmentKeyMode {
    if rng.random_range(0..2) == 0 {
        FragmentKeyMode::Unkeyed
    } else {
        FragmentKeyMode::Keyed { base: rng.random() }
    }
}

fn random_suspense_mode<R: Rng + ?Sized>(rng: &mut R) -> SuspenseMode {
    match rng.random_range(0..3) {
        0 => SuspenseMode::Resolved,
        1 => SuspenseMode::Pending,
        _ => SuspenseMode::Ready,
    }
}

fn random_wake_mutation<R: Rng + ?Sized>(rng: &mut R) -> WakeMutationSpec {
    match rng.random_range(0..4) {
        0 => WakeMutationSpec::PrependStaticRoot { tag: rng.random() },
        _ => WakeMutationSpec::None,
    }
}

fn random_selector<R: Rng + ?Sized>(rng: &mut R) -> u8 {
    if rng.random_range(0..8) == 0 {
        rng.random()
    } else {
        rng.random_range(0..=7)
    }
}

fn random_template_kind<R: Rng + ?Sized>(rng: &mut R) -> TemplateNodeKind {
    match rng.random_range(0..3) {
        0 => TemplateNodeKind::Element { tag: rng.random(), namespace: random_namespace(rng) },
        1 => TemplateNodeKind::Text(rng.random()),
        _ => TemplateNodeKind::Dynamic,
    }
}

fn random_template_attr<R: Rng + ?Sized>(rng: &mut R) -> TemplateAttrSpec {
    if rng.random_range(0..2) == 0 {
        TemplateAttrSpec::Static {
            name: rng.random(),
            value: rng.random(),
            namespace: random_namespace(rng),
        }
    } else {
        TemplateAttrSpec::Dynamic
    }
}

fn random_dynamic_kind<R: Rng + ?Sized>(rng: &mut R) -> DynamicKind {
    match rng.random_range(0..6) {
        0 => DynamicKind::Empty,
        1 => DynamicKind::Text(rng.random()),
        2 => DynamicKind::Fragment,
        3 => DynamicKind::ComponentA,
        4 => DynamicKind::ComponentB,
        _ => DynamicKind::Suspense { mode: random_suspense_mode(rng) },
    }
}

fn random_attr<R: Rng + ?Sized>(rng: &mut R) -> AttrSpec {
    let value = match rng.random_range(0..7) {
        0 => AttrValueSpec::Text(rng.random()),
        1 => AttrValueSpec::Float(rng.random()),
        2 => AttrValueSpec::Int(rng.random()),
        3 => AttrValueSpec::Bool(rng.random()),
        4 => AttrValueSpec::Any(rng.random()),
        5 => AttrValueSpec::None,
        _ => AttrValueSpec::Listener,
    };

    let namespace = if matches!(value, AttrValueSpec::Listener) {
        None
    } else {
        random_namespace(rng)
    };

    AttrSpec { name: rng.random(), namespace, value, volatile: rng.random() }
}

fn random_list_edit<R, T>(rng: &mut R, mut random_item: impl FnMut(&mut R) -> T) -> ListEdit<T>
where
    R: Rng + ?Sized,
{
    match rng.random_range(0..3) {
        0 => ListEdit::Insert { index: random_selector(rng), item: random_item(rng) },
        1 => ListEdit::Remove { index: random_selector(rng) },
        _ => ListEdit::Move { from: random_selector(rng), to: random_selector(rng) },
    }
}

fn random_template_edit<R: Rng + ?Sized>(rng: &mut R) -> TemplateEdit {
    match rng.random_range(0..4) {
        0 => TemplateEdit::SetNode { node: random_selector(rng), kind: random_template_kind(rng) },
        1 => TemplateEdit::Roots { edit: random_list_edit(rng, random_template_kind) },
        2 => TemplateEdit::Children {
            element: random_selector(rng),
            edit: random_list_edit(rng, random_template_kind),
        },
        _ => TemplateEdit::Attrs {
            element: random_selector(rng),
            edit: random_list_edit(rng, random_template_attr),
        },
    }
}

fn random_fragment_edit<R: Rng + ?Sized>(rng: &mut R) -> FragmentEdit {
    if rng.random_range(0..3) == 0 {
        FragmentEdit::KeyMode(random_fragment_key_mode(rng))
    } else {
        FragmentEdit::Children(random_list_edit(rng, random_key))
    }
}

// ---------- Structured operation generation --------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FuzzProfile {
    Mixed,
    Iterator,
    Random,
}

impl FuzzProfile {
    pub(crate) fn from_env() -> Self {
        match std::env::var("FUZZ_PROFILE")
            .unwrap_or_else(|_| "mixed".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "iterator" | "iter" => Self::Iterator,
            "random" => Self::Random,
            _ => Self::Mixed,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Mixed => "mixed",
            Self::Iterator => "iterator",
            Self::Random => "random",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IteratorScenario {
    UnkeyedAppend,
    UnkeyedRemove,
    KeyedPrepend,
    KeyedAppend,
    KeyedMiddleInsert,
    KeyedMiddleRemove,
    KeyedReplaceAll,
    KeyedMoveNearFront,
    KeyedMoveFirstToEnd,
    NestedDomlessMove,
}

impl IteratorScenario {
    pub(crate) const ALL: [Self; 10] = [
        Self::UnkeyedAppend,
        Self::UnkeyedRemove,
        Self::KeyedPrepend,
        Self::KeyedAppend,
        Self::KeyedMiddleInsert,
        Self::KeyedMiddleRemove,
        Self::KeyedReplaceAll,
        Self::KeyedMoveNearFront,
        Self::KeyedMoveFirstToEnd,
        Self::NestedDomlessMove,
    ];
}

pub(crate) fn iterator_scenario_ops(scenario: IteratorScenario, key_base: u8) -> Vec<Op> {
    match scenario {
        IteratorScenario::UnkeyedAppend => {
            let mut ops = unkeyed_fragment_with_len(2);
            ops.push(Op::Rerender);
            ops.push(fragment_insert(2, None));
            ops.push(Op::Rerender);
            ops
        }
        IteratorScenario::UnkeyedRemove => {
            let mut ops = unkeyed_fragment_with_len(3);
            ops.push(Op::Rerender);
            ops.push(fragment_remove(1));
            ops.push(Op::Rerender);
            ops
        }
        IteratorScenario::KeyedPrepend => {
            let mut ops = keyed_fragment_with_len(key_base, 3);
            ops.push(Op::Rerender);
            ops.push(fragment_insert(0, Some(key_base.wrapping_add(16))));
            ops.push(Op::Rerender);
            ops
        }
        IteratorScenario::KeyedAppend => {
            let mut ops = keyed_fragment_with_len(key_base, 3);
            ops.push(Op::Rerender);
            ops.push(fragment_insert(3, Some(key_base.wrapping_add(3))));
            ops.push(Op::Rerender);
            ops
        }
        IteratorScenario::KeyedMiddleInsert => {
            let mut ops = keyed_fragment_with_len(key_base, 3);
            ops.push(Op::Rerender);
            ops.push(fragment_insert(1, Some(key_base.wrapping_add(16))));
            ops.push(Op::Rerender);
            ops
        }
        IteratorScenario::KeyedMiddleRemove => {
            let mut ops = keyed_fragment_with_len(key_base, 4);
            ops.push(Op::Rerender);
            ops.push(fragment_remove(1));
            ops.push(Op::Rerender);
            ops
        }
        IteratorScenario::KeyedReplaceAll => {
            let mut ops = keyed_fragment_with_len(key_base, 3);
            ops.push(Op::Rerender);
            ops.push(fragment_key_mode(FragmentKeyMode::Keyed {
                base: key_base.wrapping_add(32),
            }));
            ops.push(Op::Rerender);
            ops
        }
        IteratorScenario::KeyedMoveNearFront => {
            let mut ops = keyed_fragment_with_len(key_base, 4);
            ops.push(Op::Rerender);
            ops.push(fragment_move(1, 0));
            ops.push(Op::Rerender);
            ops
        }
        IteratorScenario::KeyedMoveFirstToEnd => {
            let mut ops = keyed_fragment_with_len(key_base, 4);
            ops.push(Op::Rerender);
            ops.push(fragment_move(0, 3));
            ops.push(Op::Rerender);
            ops
        }
        IteratorScenario::NestedDomlessMove => nested_domless_move_scenario(),
    }
}

fn make_root_dynamic() -> Op {
    Op::Template {
        vnode: 0,
        edit: TemplateEdit::SetNode { node: 0, kind: TemplateNodeKind::Dynamic },
    }
}

fn fragment_insert(index: u8, item: Option<u8>) -> Op {
    Op::Fragment {
        vnode: 0,
        slot: 0,
        edit: FragmentEdit::Children(ListEdit::Insert { index, item }),
    }
}

fn fragment_remove(index: u8) -> Op {
    Op::Fragment { vnode: 0, slot: 0, edit: FragmentEdit::Children(ListEdit::Remove { index }) }
}

fn fragment_move(from: u8, to: u8) -> Op {
    Op::Fragment { vnode: 0, slot: 0, edit: FragmentEdit::Children(ListEdit::Move { from, to }) }
}

fn fragment_key_mode(mode: FragmentKeyMode) -> Op {
    Op::Fragment { vnode: 0, slot: 0, edit: FragmentEdit::KeyMode(mode) }
}

fn unkeyed_fragment_with_len(len: u8) -> Vec<Op> {
    let mut ops = Vec::with_capacity(len as usize + 1);
    ops.push(make_root_dynamic());
    for index in 0..len {
        ops.push(fragment_insert(index, None));
    }
    ops
}

fn keyed_fragment_with_len(key_base: u8, len: u8) -> Vec<Op> {
    let mut ops = Vec::with_capacity(len as usize + 1);
    ops.push(make_root_dynamic());
    for index in 0..len {
        ops.push(fragment_insert(index, Some(key_base.wrapping_add(index))));
    }
    ops
}

fn nested_domless_move_scenario() -> Vec<Op> {
    vec![
        make_root_dynamic(),
        fragment_insert(0, None),
        fragment_insert(0, None),
        fragment_insert(0, None),
        fragment_key_mode(FragmentKeyMode::Keyed { base: 0 }),
        fragment_insert(0, None),
        Op::Template {
            vnode: 6,
            edit: TemplateEdit::SetNode { node: 0, kind: TemplateNodeKind::Dynamic },
        },
        Op::Template {
            vnode: 7,
            edit: TemplateEdit::Children {
                element: 0,
                edit: ListEdit::Insert { index: 0, item: TemplateNodeKind::Dynamic },
            },
        },
        fragment_insert(0, None),
        Op::Fragment {
            vnode: 177,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Insert { index: 0, item: None }),
        },
        Op::Rerender,
        Op::Dynamic { vnode: 2, slot: 0, kind: DynamicKind::ComponentA },
        fragment_move(3, 2),
        Op::Rerender,
    ]
}

// ---------- Model operations -----------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Op {
    Rerender,
    WakeSuspense { suspense: u8 },
    WakeSuspenseNatural { suspense: u8 },
    Template { vnode: u8, edit: TemplateEdit },
    Dynamic { vnode: u8, slot: u8, kind: DynamicKind },
    DynamicAttrs { vnode: u8, slot: u8, edit: ListEdit<AttrSpec> },
    Fragment { vnode: u8, slot: u8, edit: FragmentEdit },
    Suspense { suspense: u8, mode: SuspenseMode },
    SuspenseWakeMutation { suspense: u8, mutation: WakeMutationSpec },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TemplateEdit {
    SetNode { node: u8, kind: TemplateNodeKind },
    Roots { edit: ListEdit<TemplateNodeKind> },
    Children { element: u8, edit: ListEdit<TemplateNodeKind> },
    Attrs { element: u8, edit: ListEdit<TemplateAttrSpec> },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FragmentEdit {
    KeyMode(FragmentKeyMode),
    Children(ListEdit<Option<u8>>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ListEdit<T> {
    Insert { index: u8, item: T },
    Remove { index: u8 },
    Move { from: u8, to: u8 },
}

impl Distribution<Op> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Op {
        match rng.random_range(0..18) {
            0 | 1 | 2 => Op::Rerender,
            3 => Op::WakeSuspense { suspense: random_selector(rng) },
            4 => Op::WakeSuspenseNatural { suspense: random_selector(rng) },
            5 => Op::SuspenseWakeMutation {
                suspense: random_selector(rng),
                mutation: random_wake_mutation(rng),
            },
            6 | 7 | 8 | 9 => {
                Op::Template { vnode: random_selector(rng), edit: random_template_edit(rng) }
            }
            10 => Op::Dynamic {
                vnode: random_selector(rng),
                slot: random_selector(rng),
                kind: random_dynamic_kind(rng),
            },
            11 => Op::Suspense { suspense: random_selector(rng), mode: random_suspense_mode(rng) },
            12 | 13 | 14 => Op::Fragment {
                vnode: random_selector(rng),
                slot: random_selector(rng),
                edit: random_fragment_edit(rng),
            },
            _ => Op::DynamicAttrs {
                vnode: random_selector(rng),
                slot: random_selector(rng),
                edit: random_list_edit(rng, random_attr),
            },
        }
    }
}

thread_local! {
    static MODEL: RefCell<Model> = RefCell::new(Model::initial());
    static SUSPENSE_READY_RELEASED: RefCell<Vec<SuspenseReadyKey>> = RefCell::new(Vec::new());
    static SUSPENSE_READY_WAKERS: RefCell<Vec<(SuspenseReadyKey, Waker)>> = RefCell::new(Vec::new());
    static REGISTER_SUSPENSE_READY_SENDERS: Cell<bool> = Cell::new(true);
}

pub(crate) fn read_model() -> Model {
    MODEL.with(|m| m.borrow().clone())
}

pub(crate) fn with_model<R>(f: impl FnOnce(&mut Model) -> R) -> R {
    MODEL.with(|m| f(&mut m.borrow_mut()))
}

fn suspense_ready_released(key: SuspenseReadyKey) -> bool {
    REGISTER_SUSPENSE_READY_SENDERS.with(|enabled| {
        enabled.get() && SUSPENSE_READY_RELEASED.with(|released| released.borrow().contains(&key))
    })
}

fn register_suspense_ready_waker(key: SuspenseReadyKey, waker: Waker) {
    REGISTER_SUSPENSE_READY_SENDERS.with(|enabled| {
        if enabled.get() {
            SUSPENSE_READY_WAKERS.with(|wakers| wakers.borrow_mut().push((key, waker)));
        }
    });
}

pub(crate) fn release_suspense_ready_task(key: SuspenseReadyKey) {
    SUSPENSE_READY_RELEASED.with(|released| {
        if !released.borrow().contains(&key) {
            released.borrow_mut().push(key);
        }
    });
    SUSPENSE_READY_WAKERS.with(|wakers| {
        let mut wakers = wakers.borrow_mut();
        let mut index = 0;
        while index < wakers.len() {
            if wakers[index].0 == key {
                let (_, waker) = wakers.swap_remove(index);
                waker.wake();
            } else {
                index += 1;
            }
        }
    });
}

pub(crate) fn selected_registered_ready_suspense_key(selector: u8) -> Option<SuspenseReadyKey> {
    let registered = SUSPENSE_READY_WAKERS.with(|wakers| {
        let mut keys = Vec::new();
        for (key, _) in wakers.borrow().iter() {
            if !keys.contains(key) {
                keys.push(*key);
            }
        }
        keys
    });

    let mut ready = Vec::new();
    read_model().root.collect_ready_suspense_keys(&mut ready);
    ready.retain(|key| registered.contains(key));
    select(ready, selector)
}

pub(crate) fn clear_suspense_ready_tasks() {
    SUSPENSE_READY_RELEASED.with(|released| released.borrow_mut().clear());
    SUSPENSE_READY_WAKERS.with(|wakers| wakers.borrow_mut().clear());
}

struct SuspenseReadyRegistrationGuard {
    previous: bool,
}

impl Drop for SuspenseReadyRegistrationGuard {
    fn drop(&mut self) {
        REGISTER_SUSPENSE_READY_SENDERS.with(|enabled| enabled.set(self.previous));
    }
}

pub(crate) fn without_suspense_ready_registration<R>(f: impl FnOnce() -> R) -> R {
    let _guard = REGISTER_SUSPENSE_READY_SENDERS.with(|enabled| {
        let previous = enabled.replace(false);
        SuspenseReadyRegistrationGuard { previous }
    });
    f()
}

pub(crate) struct SuspenseReadyFuture {
    pub(crate) key: SuspenseReadyKey,
}

impl Future for SuspenseReadyFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let key = self.key;
        if suspense_ready_released(key) {
            Poll::Ready(())
        } else {
            register_suspense_ready_waker(key, cx.waker().clone());
            Poll::Pending
        }
    }
}

pub(crate) fn apply_op_to_model(model: &mut Model, op: &Op) {
    if matches!(op, Op::Rerender) {
        return;
    }

    let can_grow = model.can_grow();
    match op {
        Op::Rerender => {}
        Op::WakeSuspense { suspense } | Op::WakeSuspenseNatural { suspense } => {
            if let Some(key) = model.selected_ready_suspense_key(*suspense) {
                model.resolve_ready_suspense(key);
            }
        }
        Op::Template { vnode, edit } => {
            let vnode = model.selected_vnode_mut(*vnode);
            apply_template_edit(vnode, edit, can_grow);
            vnode.normalize_in_place();
        }
        Op::Dynamic { vnode, slot, kind } => {
            let mut next_suspense_id = model.next_suspense_id;
            {
                let vnode = model.selected_vnode_mut(*vnode);
                if !vnode.dynamics.is_empty() {
                    let index = *slot as usize % vnode.dynamics.len();
                    if can_grow || matches!(kind, DynamicKind::Empty | DynamicKind::Text(_)) {
                        vnode.dynamics[index].set_kind(kind, &mut next_suspense_id);
                    }
                }
                vnode.normalize_in_place();
            }
            model.next_suspense_id = next_suspense_id;
        }
        Op::DynamicAttrs { vnode, slot, edit } => {
            let vnode = model.selected_vnode_mut(*vnode);
            if !vnode.attrs.is_empty() {
                let index = *slot as usize % vnode.attrs.len();
                apply_attr_list_edit(&mut vnode.attrs[index], edit);
                sort_attrs(index, &mut vnode.attrs[index]);
            }
            vnode.normalize_in_place();
        }
        Op::Fragment { vnode, slot, edit } => {
            let vnode = model.selected_vnode_mut(*vnode);
            apply_fragment_edit(vnode, *slot, edit, can_grow);
            vnode.normalize_in_place();
        }
        Op::Suspense { suspense, mode } => {
            model.set_selected_suspense_mode(*suspense, *mode);
        }
        Op::SuspenseWakeMutation { suspense, mutation } => {
            model.set_selected_suspense_wake_mutation(*suspense, *mutation);
        }
    }
}

pub(crate) fn apply_to_model(op: &Op) {
    with_model(|model| apply_op_to_model(model, op));
}

fn apply_template_edit(vnode: &mut VNodeSpec, edit: &TemplateEdit, can_grow: bool) {
    match edit {
        TemplateEdit::SetNode { node, kind } => {
            if let Some(path) = select(vnode.template.node_paths(), *node) {
                if let Some(node) = vnode.template.node_mut(&path) {
                    node.set_kind(kind);
                }
            }
        }
        TemplateEdit::Roots { edit } => {
            apply_template_node_list_edit(&mut vnode.template.roots, edit, 1, MAX_ROOTS, can_grow);
        }
        TemplateEdit::Children { element, edit } => {
            if let Some(path) = select(vnode.template.element_paths(), *element) {
                if let Some(TemplateNodeSpec::Element { children, .. }) =
                    vnode.template.element_mut(&path)
                {
                    apply_template_node_list_edit(children, edit, 0, MAX_CHILDREN, can_grow);
                }
            }
        }
        TemplateEdit::Attrs { element, edit } => {
            if let Some(path) = select(vnode.template.element_paths(), *element) {
                if let Some(TemplateNodeSpec::Element { attrs, .. }) =
                    vnode.template.element_mut(&path)
                {
                    apply_template_attr_list_edit(attrs, edit);
                }
            }
        }
    }
}

fn apply_fragment_edit(vnode: &mut VNodeSpec, slot: u8, edit: &FragmentEdit, can_grow: bool) {
    match edit {
        FragmentEdit::KeyMode(mode) => {
            if let Some(children) = selected_fragment_mut(vnode, slot) {
                apply_fragment_key_mode(children, mode);
            }
        }
        FragmentEdit::Children(ListEdit::Insert { index, item }) => {
            if can_grow {
                if let Some(children) = selected_fragment_mut(vnode, slot) {
                    insert_fragment_child(children, *index, *item);
                }
            }
        }
        FragmentEdit::Children(ListEdit::Remove { index }) => {
            if let Some(children) = selected_existing_fragment_mut(vnode, slot) {
                remove_selected(children, *index, 0);
            }
        }
        FragmentEdit::Children(ListEdit::Move { from, to }) => {
            if let Some(children) = selected_existing_fragment_mut(vnode, slot) {
                move_selected(children, *from, *to);
            }
        }
    }
}

fn apply_template_node_list_edit(
    nodes: &mut Vec<TemplateNodeSpec>,
    edit: &ListEdit<TemplateNodeKind>,
    min_len: usize,
    max_len: usize,
    can_grow: bool,
) {
    match edit {
        ListEdit::Insert { index, item } => {
            if can_grow && nodes.len() < max_len {
                let index = insert_index(nodes.len(), *index);
                nodes.insert(index, TemplateNodeSpec::from_kind(item));
            }
        }
        ListEdit::Remove { index } => {
            remove_selected(nodes, *index, min_len);
        }
        ListEdit::Move { from, to } => {
            move_selected(nodes, *from, *to);
        }
    }
}

fn apply_template_attr_list_edit(
    attrs: &mut Vec<TemplateAttrSpec>,
    edit: &ListEdit<TemplateAttrSpec>,
) {
    match edit {
        ListEdit::Insert { index, item } => {
            if attrs.len() < MAX_TEMPLATE_ATTRS {
                let index = insert_index(attrs.len(), *index);
                attrs.insert(index, item.clone());
            }
        }
        ListEdit::Remove { index } => {
            remove_selected(attrs, *index, 0);
        }
        ListEdit::Move { from, to } => {
            move_selected(attrs, *from, *to);
        }
    }
}

fn apply_attr_list_edit(attrs: &mut Vec<AttrSpec>, edit: &ListEdit<AttrSpec>) {
    match edit {
        ListEdit::Insert { index, item } => {
            if attrs.len() < MAX_DYNAMIC_ATTRS {
                let index = insert_index(attrs.len(), *index);
                attrs.insert(index, item.clone());
            }
        }
        ListEdit::Remove { index } => {
            remove_selected(attrs, *index, 0);
        }
        ListEdit::Move { from, to } => {
            move_selected(attrs, *from, *to);
        }
    }
}

fn insert_index(len: usize, selector: u8) -> usize {
    selector as usize % (len + 1)
}

fn remove_selected<T>(items: &mut Vec<T>, selector: u8, min_len: usize) {
    if items.len() <= min_len {
        return;
    }
    let index = selector as usize % items.len();
    items.remove(index);
}

fn move_selected<T>(items: &mut Vec<T>, from: u8, to: u8) {
    if items.len() <= 1 {
        return;
    }
    let from = from as usize % items.len();
    let item = items.remove(from);
    let to = to as usize % (items.len() + 1);
    items.insert(to, item);
}

fn selected_dynamic_mut(vnode: &mut VNodeSpec, selector: u8) -> Option<&mut DynamicSpec> {
    if vnode.dynamics.is_empty() {
        return None;
    }
    let index = selector as usize % vnode.dynamics.len();
    Some(&mut vnode.dynamics[index])
}

fn selected_fragment_mut(vnode: &mut VNodeSpec, selector: u8) -> Option<&mut Vec<VNodeSpec>> {
    let dynamic = selected_dynamic_mut(vnode, selector)?;
    if !matches!(dynamic, DynamicSpec::Fragment(_)) {
        *dynamic = DynamicSpec::Fragment(Vec::new());
    }
    let DynamicSpec::Fragment(children) = dynamic else {
        unreachable!();
    };
    Some(children)
}

fn selected_existing_fragment_mut(
    vnode: &mut VNodeSpec,
    selector: u8,
) -> Option<&mut Vec<VNodeSpec>> {
    match selected_dynamic_mut(vnode, selector)? {
        DynamicSpec::Fragment(children) => Some(children),
        _ => None,
    }
}

fn apply_fragment_key_mode(children: &mut [VNodeSpec], mode: &FragmentKeyMode) {
    for (index, child) in children.iter_mut().enumerate() {
        child.key = match mode {
            FragmentKeyMode::Unkeyed => None,
            FragmentKeyMode::Keyed { base } => Some(base.wrapping_add(index as u8)),
        };
    }
}

fn insert_fragment_child(children: &mut Vec<VNodeSpec>, index: u8, key: Option<u8>) {
    if children.len() >= MAX_FRAGMENT_CHILDREN {
        return;
    }
    let mut child = VNodeSpec::minimal();
    child.key = fragment_child_key(children, key);
    let index = insert_index(children.len(), index);
    children.insert(index, child);
}

fn fragment_child_key(children: &[VNodeSpec], requested: Option<u8>) -> Option<u8> {
    match children.first().and_then(|child| child.key) {
        Some(_) => Some(unique_fragment_key(children, requested.unwrap_or(0))),
        None if children.is_empty() => requested,
        None => None,
    }
}

fn unique_fragment_key(children: &[VNodeSpec], mut candidate: u8) -> u8 {
    while children.iter().any(|child| child.key == Some(candidate)) {
        candidate = candidate.wrapping_add(1);
    }
    candidate
}
