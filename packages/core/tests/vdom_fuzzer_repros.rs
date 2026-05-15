#![allow(dead_code, non_snake_case)]

#[path = "vdom_fuzzer_repros/harness.rs"]
mod harness;
#[path = "vdom_fuzzer_repros/model.rs"]
mod model;
#[path = "vdom_fuzzer_repros/ops.rs"]
mod ops;
#[path = "vdom_fuzzer_repros/vdom.rs"]
mod vdom;

use harness::{Harness, apply_step};
use model::{
    AttrSpec, AttrValueSpec, DynamicKind, FragmentKeyMode, SuspenseMode, TemplateAttrSpec,
    TemplateNodeKind, WakeMutationSpec,
};
use ops::{FragmentEdit, ListEdit, Op, TemplateEdit};

fn replay(ops: impl IntoIterator<Item = Op>) {
    let mut harness = Harness::fresh();
    for op in ops {
        if let Err(error) = apply_step(&mut harness, &op) {
            std::mem::forget(harness);
            panic!("{error}");
        }
    }
}

#[test]
fn repro_suspense_replay_scope_runtime_panic() {
    replay([
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::SetNode { node: 0, kind: TemplateNodeKind::Dynamic },
        },
        Op::Dynamic {
            vnode: 0,
            slot: 0,
            kind: DynamicKind::Suspense { mode: SuspenseMode::Resolved },
        },
        Op::Template {
            vnode: 3,
            edit: TemplateEdit::SetNode { node: 0, kind: TemplateNodeKind::Dynamic },
        },
        Op::Dynamic {
            vnode: 7,
            slot: 0,
            kind: DynamicKind::Suspense { mode: SuspenseMode::Ready },
        },
        Op::Rerender,
        Op::Suspense { suspense: 0, mode: SuspenseMode::Pending },
        Op::Template {
            vnode: 7,
            edit: TemplateEdit::Roots {
                edit: ListEdit::Insert { index: 0, item: TemplateNodeKind::Dynamic },
            },
        },
        Op::Rerender,
        Op::Suspense { suspense: 0, mode: SuspenseMode::Resolved },
        Op::WakeSuspense { suspense: 0 },
    ]);
}

#[test]
fn repro_suspense_wake_after_parent_root_insert_iterator_panic() {
    replay([
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::SetNode { node: 0, kind: TemplateNodeKind::Dynamic },
        },
        Op::Dynamic {
            vnode: 0,
            slot: 0,
            kind: DynamicKind::Suspense { mode: SuspenseMode::Resolved },
        },
        Op::Template {
            vnode: 3,
            edit: TemplateEdit::SetNode { node: 0, kind: TemplateNodeKind::Dynamic },
        },
        Op::Dynamic {
            vnode: 7,
            slot: 0,
            kind: DynamicKind::Suspense { mode: SuspenseMode::Ready },
        },
        Op::Rerender,
        Op::Suspense { suspense: 0, mode: SuspenseMode::Pending },
        Op::Template {
            vnode: 7,
            edit: TemplateEdit::Roots {
                edit: ListEdit::Insert { index: 0, item: TemplateNodeKind::Dynamic },
            },
        },
        Op::Rerender,
        Op::Suspense { suspense: 0, mode: SuspenseMode::Resolved },
        Op::Rerender,
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::Roots {
                edit: ListEdit::Insert { index: 0, item: TemplateNodeKind::Dynamic },
            },
        },
        Op::WakeSuspense { suspense: 0 },
    ]);
}

#[test]
fn repro_nested_suspense_wake_after_parent_attr_and_child_edit_iterator_panic() {
    replay([
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::Roots {
                edit: ListEdit::Insert { index: 0, item: TemplateNodeKind::Dynamic },
            },
        },
        Op::Dynamic {
            vnode: 0,
            slot: 0,
            kind: DynamicKind::Suspense { mode: SuspenseMode::Resolved },
        },
        Op::Template {
            vnode: 3,
            edit: TemplateEdit::SetNode { node: 0, kind: TemplateNodeKind::Dynamic },
        },
        Op::Dynamic {
            vnode: 7,
            slot: 0,
            kind: DynamicKind::Suspense { mode: SuspenseMode::Ready },
        },
        Op::Rerender,
        Op::Suspense { suspense: 0, mode: SuspenseMode::Ready },
        Op::Rerender,
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::Attrs {
                element: 0,
                edit: ListEdit::Insert { index: 0, item: TemplateAttrSpec::Dynamic },
            },
        },
        Op::WakeSuspense { suspense: 0 },
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::Children {
                element: 0,
                edit: ListEdit::Insert { index: 0, item: TemplateNodeKind::Dynamic },
            },
        },
        Op::Rerender,
        Op::WakeSuspense { suspense: 0 },
    ]);
}

#[test]
fn repro_natural_wake_nested_suspense_hidden_mutation_parent_link_panic() {
    replay([
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::SetNode { node: 0, kind: TemplateNodeKind::Dynamic },
        },
        Op::Dynamic {
            vnode: 0,
            slot: 0,
            kind: DynamicKind::Suspense { mode: SuspenseMode::Resolved },
        },
        Op::Template {
            vnode: 3,
            edit: TemplateEdit::SetNode { node: 0, kind: TemplateNodeKind::Dynamic },
        },
        Op::Dynamic {
            vnode: 7,
            slot: 0,
            kind: DynamicKind::Suspense { mode: SuspenseMode::Ready },
        },
        Op::SuspenseWakeMutation {
            suspense: 1,
            mutation: WakeMutationSpec::PrependStaticRoot { tag: 42 },
        },
        Op::Rerender,
        Op::Suspense { suspense: 0, mode: SuspenseMode::Ready },
        Op::Rerender,
        Op::WakeSuspenseNatural { suspense: 1 },
        Op::WakeSuspenseNatural { suspense: 0 },
    ]);
}

#[test]
fn repro_root_dynamic_suspense_then_static_text_underflows() {
    replay([
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::SetNode { node: 0, kind: TemplateNodeKind::Dynamic },
        },
        Op::Dynamic {
            vnode: 206,
            slot: 3,
            kind: DynamicKind::Suspense { mode: SuspenseMode::Resolved },
        },
        Op::Template {
            vnode: 5,
            edit: TemplateEdit::SetNode { node: 2, kind: TemplateNodeKind::Dynamic },
        },
        Op::Rerender,
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::SetNode { node: 3, kind: TemplateNodeKind::Text(0) },
        },
        Op::Rerender,
    ]);
}

#[test]
fn repro_nested_suspense_slot_static_child_iterator_panic() {
    replay([
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::Children {
                element: 7,
                edit: ListEdit::Insert { index: 16, item: TemplateNodeKind::Text(68) },
            },
        },
        Op::Template {
            vnode: 5,
            edit: TemplateEdit::Roots {
                edit: ListEdit::Insert { index: 1, item: TemplateNodeKind::Text(24) },
            },
        },
        Op::Template {
            vnode: 1,
            edit: TemplateEdit::SetNode { node: 143, kind: TemplateNodeKind::Dynamic },
        },
        Op::Template {
            vnode: 3,
            edit: TemplateEdit::Children {
                element: 3,
                edit: ListEdit::Insert {
                    index: 6,
                    item: TemplateNodeKind::Element { tag: 66, namespace: None },
                },
            },
        },
        Op::Dynamic {
            vnode: 4,
            slot: 4,
            kind: DynamicKind::Suspense { mode: SuspenseMode::Ready },
        },
        Op::Template {
            vnode: 7,
            edit: TemplateEdit::SetNode { node: 7, kind: TemplateNodeKind::Dynamic },
        },
        Op::Template {
            vnode: 88,
            edit: TemplateEdit::SetNode { node: 6, kind: TemplateNodeKind::Dynamic },
        },
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::Children {
                element: 1,
                edit: ListEdit::Insert { index: 5, item: TemplateNodeKind::Dynamic },
            },
        },
        Op::Dynamic { vnode: 4, slot: 2, kind: DynamicKind::ComponentB },
        Op::WakeSuspense { suspense: 120 },
        Op::Dynamic {
            vnode: 1,
            slot: 5,
            kind: DynamicKind::Suspense { mode: SuspenseMode::Ready },
        },
        Op::Template {
            vnode: 6,
            edit: TemplateEdit::SetNode { node: 7, kind: TemplateNodeKind::Dynamic },
        },
        Op::WakeSuspense { suspense: 4 },
        Op::Template {
            vnode: 5,
            edit: TemplateEdit::SetNode {
                node: 7,
                kind: TemplateNodeKind::Element { tag: 0, namespace: Some(0) },
            },
        },
        Op::Rerender,
    ]);
}

#[test]
fn repro_nested_suspense_wake_replaces_inner_fallback_root() {
    replay([
        Op::Template {
            vnode: 183,
            edit: TemplateEdit::Roots {
                edit: ListEdit::Insert { index: 0, item: TemplateNodeKind::Dynamic },
            },
        },
        Op::Dynamic {
            vnode: 0,
            slot: 1,
            kind: DynamicKind::Suspense { mode: SuspenseMode::Pending },
        },
        Op::Template {
            vnode: 7,
            edit: TemplateEdit::Roots {
                edit: ListEdit::Insert { index: 1, item: TemplateNodeKind::Dynamic },
            },
        },
        Op::Suspense { suspense: 4, mode: SuspenseMode::Resolved },
        Op::Dynamic {
            vnode: 3,
            slot: 2,
            kind: DynamicKind::Suspense { mode: SuspenseMode::Ready },
        },
        Op::Rerender,
        Op::Suspense { suspense: 0, mode: SuspenseMode::Ready },
        Op::Rerender,
        Op::Suspense { suspense: 1, mode: SuspenseMode::Resolved },
        Op::WakeSuspense { suspense: 2 },
    ]);
}

#[test]
fn repro_keyed_fragment_moves_nested_child_after_component_insert_iterator_panic() {
    replay([
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::SetNode { node: 0, kind: TemplateNodeKind::Dynamic },
        },
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Insert { index: 0, item: None }),
        },
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Insert { index: 0, item: None }),
        },
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Insert { index: 0, item: None }),
        },
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::KeyMode(FragmentKeyMode::Keyed { base: 0 }),
        },
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Insert { index: 0, item: None }),
        },
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
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Insert { index: 0, item: None }),
        },
        Op::Fragment {
            vnode: 177,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Insert { index: 0, item: None }),
        },
        Op::Rerender,
        Op::Dynamic { vnode: 2, slot: 0, kind: DynamicKind::ComponentA },
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Move { from: 3, to: 2 }),
        },
        Op::Rerender,
    ]);
}

#[test]
fn repro_keyed_fragment_remove_after_domless_child_move_iterator_panic() {
    replay([
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::SetNode { node: 0, kind: TemplateNodeKind::Dynamic },
        },
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Insert { index: 0, item: None }),
        },
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Insert { index: 0, item: None }),
        },
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Insert { index: 0, item: None }),
        },
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::KeyMode(FragmentKeyMode::Keyed { base: 0 }),
        },
        Op::Template {
            vnode: 6,
            edit: TemplateEdit::SetNode { node: 0, kind: TemplateNodeKind::Dynamic },
        },
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Insert { index: 0, item: None }),
        },
        Op::Rerender,
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Move { from: 3, to: 2 }),
        },
        Op::Fragment {
            vnode: 0,
            slot: 0,
            edit: FragmentEdit::Children(ListEdit::Remove { index: 0 }),
        },
        Op::Rerender,
    ]);
}

#[test]
fn repro_template_hash_root_sibling_vs_nested_child_diverges() {
    replay([
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::Roots {
                edit: ListEdit::Insert { index: 0, item: TemplateNodeKind::Dynamic },
            },
        },
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::Roots {
                edit: ListEdit::Insert { index: 0, item: TemplateNodeKind::Dynamic },
            },
        },
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::Roots { edit: ListEdit::Remove { index: 0 } },
        },
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::SetNode { node: 5, kind: TemplateNodeKind::Text(36) },
        },
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::SetNode {
                node: 0,
                kind: TemplateNodeKind::Element { tag: 0, namespace: None },
            },
        },
        Op::Rerender,
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::Roots { edit: ListEdit::Remove { index: 1 } },
        },
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::Children {
                element: 0,
                edit: ListEdit::Insert { index: 0, item: TemplateNodeKind::Text(36) },
            },
        },
        Op::Rerender,
    ]);
}

#[test]
fn repro_dynamic_attribute_shadowing_diverges() {
    replay([
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::Attrs {
                element: 0,
                edit: ListEdit::Insert { index: 0, item: TemplateAttrSpec::Dynamic },
            },
        },
        Op::Template {
            vnode: 0,
            edit: TemplateEdit::Attrs {
                element: 0,
                edit: ListEdit::Insert { index: 0, item: TemplateAttrSpec::Dynamic },
            },
        },
        Op::DynamicAttrs {
            vnode: 0,
            slot: 7,
            edit: ListEdit::Insert {
                index: 0,
                item: AttrSpec {
                    name: 0,
                    namespace: None,
                    value: AttrValueSpec::Int(0),
                    volatile: false,
                },
            },
        },
        Op::DynamicAttrs {
            vnode: 0,
            slot: 0,
            edit: ListEdit::Insert {
                index: 0,
                item: AttrSpec {
                    name: 0,
                    namespace: None,
                    value: AttrValueSpec::None,
                    volatile: true,
                },
            },
        },
        Op::Rerender,
    ]);
}
