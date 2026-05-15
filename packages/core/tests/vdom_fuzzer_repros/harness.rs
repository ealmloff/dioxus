use crate::{
    model::*,
    ops::{
        Op, apply_to_model, clear_suspense_ready_tasks, read_model, release_suspense_ready_task,
        selected_registered_ready_suspense_key, with_model, without_suspense_ready_registration,
    },
    vdom::App,
};
use dioxus_core::{ScopeId, VirtualDom};
use dioxus_renderer_oracle::{RendererOracle, SnapshotNode, fresh_snapshot, panic_message};

// ---------- Harness -------------------------------------------------------------------------

pub(crate) struct Harness {
    vdom: VirtualDom,
    incremental: RendererOracle,
    pending_app_render: bool,
}

impl Harness {
    pub(crate) fn fresh() -> Self {
        clear_suspense_ready_tasks();
        with_model(|model| *model = Model::initial());
        let mut vdom = VirtualDom::new(App);
        let mut incremental = RendererOracle::new();
        vdom.rebuild(&mut incremental);
        incremental.assert_stack_clean();
        Self { vdom, incremental, pending_app_render: false }
    }
}

fn fresh_render() -> Vec<SnapshotNode> {
    without_suspense_ready_registration(|| fresh_snapshot(App))
}

fn render_model_with_ssr(model: &Model) -> Result<String, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        without_suspense_ready_registration(|| {
            with_model(|global| *global = model.clone());
            let mut vdom = VirtualDom::new(App);
            vdom.rebuild_in_place();
            dioxus_ssr::render(&vdom)
        })
    }))
    .map_err(|payload| format!("panic in SSR render: {}", panic_message(&payload)))
}

fn print_html_line(label: &str, rendered: &Result<String, String>) {
    match rendered {
        Ok(html) => println!("    {label:<7} {html}"),
        Err(err) => println!("    {label:<7} <{err}>"),
    }
}

pub(crate) fn print_ssr_diff_trace(ops: &[Op], minimized_error: &str) {
    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    println!();
    println!("ssr replay:");

    let mut state = Harness::fresh();
    let mut current_model = Model::initial();
    let mut current_html = render_model_with_ssr(&current_model);

    println!("  initial");
    println!("    model: {current_model:?}");
    print_html_line("html:", &current_html);

    let mut reproduced_error = None;
    for (index, op) in ops.iter().enumerate() {
        with_model(|global| *global = current_model.clone());

        println!();
        println!("  step {index}");
        println!("    op:     {op:?}");
        print_html_line("before:", &current_html);

        match apply_op(&mut state, op) {
            Ok(()) => {
                let next_model = read_model();
                let next_html = render_model_with_ssr(&next_model);
                print_html_line("after:", &next_html);
                println!("    status: ok");
                current_model = next_model;
                current_html = next_html;
            }
            Err(err) => {
                let next_model = read_model();
                let next_html = render_model_with_ssr(&next_model);
                print_html_line("after:", &next_html);
                println!("    error:  {err}");
                reproduced_error = Some(err);
                break;
            }
        }
    }

    if reproduced_error.is_none() {
        println!();
        println!("  replay completed without reproducing the minimized error:");
        println!("    {minimized_error}");
    }
    std::panic::set_hook(panic_hook);
}

pub(crate) fn apply_step(state: &mut Harness, op: &Op) -> Result<(), String> {
    apply_op(state, op)
}

fn apply_op(state: &mut Harness, op: &Op) -> Result<(), String> {
    match op {
        Op::Rerender => render_and_assert(state),
        Op::WakeSuspense { suspense } => {
            let Some(key) = read_model().selected_ready_suspense_key(*suspense) else {
                return Ok(());
            };
            apply_to_model(op);
            release_suspense_ready_task(key);
            render_and_assert(state)
        }
        Op::WakeSuspenseNatural { suspense } => {
            let Some(key) = selected_registered_ready_suspense_key(*suspense) else {
                return Ok(());
            };
            with_model(|model| model.resolve_ready_suspense(key));
            release_suspense_ready_task(key);
            let compare_fresh = !state.pending_app_render;
            render_natural_and_assert(state, compare_fresh)
        }
        _ => {
            apply_to_model(op);
            if op_requires_app_render(op) {
                state.pending_app_render = true;
            }
            Ok(())
        }
    }
}

fn op_requires_app_render(op: &Op) -> bool {
    matches!(
        op,
        Op::Template { .. }
            | Op::Dynamic { .. }
            | Op::DynamicAttrs { .. }
            | Op::Fragment { .. }
            | Op::Suspense { .. }
    )
}

fn render_once(
    state: &mut Harness,
    mark_app_dirty: bool,
    assert_matches_vdom: bool,
    label: &'static str,
) -> Result<Vec<SnapshotNode>, String> {
    if mark_app_dirty {
        state.vdom.mark_dirty(ScopeId::APP);
    }
    let render_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Synchronous transitions (e.g. suspense state changes driven by the model) may
        // queue follow-on dirty scopes through the scheduler channel during a render.
        // The fix branch has a public dirty-scope check for this; the main-branch
        // repro corpus uses a small bounded drain so the minimized cases can replay
        // against main without exposing that internal scheduler state.
        let mut snap = Vec::new();
        for _ in 0..16 {
            state.vdom.process_events();
            state.vdom.render_immediate(&mut state.incremental);
            state.incremental.assert_stack_clean();
            snap = state.incremental.snapshot();
        }
        if assert_matches_vdom {
            state.incremental.assert_matches_vdom(&state.vdom);
        }
        snap
    }));

    match render_result {
        Ok(t) => Ok(t),
        Err(payload) => Err(format!("panic in {label}: {}", panic_message(&payload),)),
    }
}

fn render_and_assert(state: &mut Harness) -> Result<(), String> {
    let incremental = render_once(state, true, true, "incremental render")?;
    let stable = render_once(state, true, true, "no-change re-render")?;
    assert_matches_stable_and_fresh(incremental, stable)?;
    state.pending_app_render = false;
    Ok(())
}

fn render_natural_and_assert(state: &mut Harness, compare_fresh: bool) -> Result<(), String> {
    let incremental = render_once(state, false, true, "natural incremental render")?;
    let stable = render_once(state, false, true, "natural no-change re-render")?;
    if compare_fresh {
        assert_matches_stable_and_fresh(incremental, stable)
    } else {
        assert_matches_stable(incremental, stable)
    }
}

fn assert_matches_stable_and_fresh(
    incremental: Vec<SnapshotNode>,
    stable: Vec<SnapshotNode>,
) -> Result<(), String> {
    assert_matches_stable(incremental.clone(), stable)?;

    let fresh_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(fresh_render));
    let fresh = match fresh_result {
        Ok(t) => t,
        Err(payload) => {
            return Err(format!(
                "panic in fresh rebuild: {}",
                panic_message(&payload),
            ));
        }
    };

    if incremental != fresh {
        return Err(format!(
            "incremental tree diverged from a fresh rebuild\n\
             incremental: {incremental:#?}\n\
             fresh:       {fresh:#?}"
        ));
    }
    Ok(())
}

fn assert_matches_stable(
    incremental: Vec<SnapshotNode>,
    stable: Vec<SnapshotNode>,
) -> Result<(), String> {
    if stable != incremental {
        return Err(format!(
            "no-change re-render changed the rendered tree\n\
             before: {incremental:#?}\n\
             after:  {stable:#?}"
        ));
    }
    Ok(())
}

#[cfg(any())]
mod tests {
    use super::*;
    use crate::{
        model::{
            AttrSpec, AttrValueSpec, DynamicKind, FragmentKeyMode, SuspenseMode, TemplateAttrSpec,
            TemplateNodeKind, WakeMutationSpec,
        },
        ops::{FragmentEdit, IteratorScenario, ListEdit, TemplateEdit, iterator_scenario_ops},
    };

    fn replay_ops(ops: impl IntoIterator<Item = Op>) {
        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn suspense_replay_does_not_duplicate_promoted_children() {
        let ops = [
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
        ];

        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn suspense_wake_after_parent_root_insert_does_not_duplicate_promoted_children() {
        let ops = [
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
        ];

        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn nested_suspense_wake_after_parent_attr_and_child_edit_does_not_duplicate_children() {
        let ops = [
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
        ];

        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn natural_wake_unmounted_ready_suspense_is_noop() {
        let ops = [
            Op::Template {
                vnode: 3,
                edit: TemplateEdit::Children {
                    element: 0,
                    edit: ListEdit::Insert { index: 5, item: TemplateNodeKind::Dynamic },
                },
            },
            Op::Dynamic {
                vnode: 5,
                slot: 2,
                kind: DynamicKind::Suspense { mode: SuspenseMode::Ready },
            },
            Op::WakeSuspenseNatural { suspense: 3 },
        ];

        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn natural_wake_after_unrendered_parent_edit_does_not_compare_fresh_model() {
        let ops = [
            Op::Template {
                vnode: 2,
                edit: TemplateEdit::Roots {
                    edit: ListEdit::Insert { index: 4, item: TemplateNodeKind::Dynamic },
                },
            },
            Op::Dynamic {
                vnode: 6,
                slot: 4,
                kind: DynamicKind::Suspense { mode: SuspenseMode::Ready },
            },
            Op::Rerender,
            Op::Template {
                vnode: 2,
                edit: TemplateEdit::Roots {
                    edit: ListEdit::Insert { index: 5, item: TemplateNodeKind::Text(110) },
                },
            },
            Op::WakeSuspenseNatural { suspense: 0 },
            Op::Rerender,
        ];

        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn natural_wake_nested_suspense_applies_hidden_wake_mutation() {
        let ops = [
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
        ];

        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn template_hash_distinguishes_root_sibling_from_nested_child() {
        let ops = [
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
        ];

        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn dynamic_attribute_shadowing_survives_no_change_rerender() {
        let ops = [
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
        ];

        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn root_dynamic_suspense_then_static_text_survives_no_change_rerender() {
        let ops = [
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
        ];

        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn nested_suspense_slot_static_child_survives_no_change_rerender() {
        let ops = [
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
        ];

        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn nested_suspense_wake_replaces_inner_fallback_root() {
        let ops = [
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
        ];

        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn keyed_fragment_moves_nested_child_after_component_insert() {
        let ops = [
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
        ];

        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn keyed_fragment_remove_after_domless_child_move_keeps_parent_links() {
        let ops = [
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
        ];

        let mut harness = Harness::fresh();
        for op in ops {
            apply_op(&mut harness, &op).unwrap();
        }
    }

    #[test]
    fn iterator_scenarios_replay() {
        for scenario in IteratorScenario::ALL {
            replay_ops(iterator_scenario_ops(scenario, 0));
        }
    }
}
