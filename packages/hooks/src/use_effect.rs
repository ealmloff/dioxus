use std::{cell::Cell, rc::Rc};

use dioxus_core::*;
use futures_util::StreamExt;

use crate::use_callback;

#[doc = include_str!("../docs/side_effects.md")]
#[doc = include_str!("../docs/rules_of_hooks.md")]
#[track_caller]
pub fn use_effect(mut callback: impl FnMut() + 'static) -> Effect {
    let callback = use_callback(move |_| callback());
    let location = std::panic::Location::caller();
    use_hook(|| Effect::new_with_location(move || callback(()), location))
}

/// A handle to an effect.
#[derive(Clone, Copy)]
pub struct Effect {
    rc: ReactiveContext,
}

impl Effect {
    /// Create a new effect that runs after the next render and reruns whenever any reactive value
    /// it reads changes.
    ///
    /// # Example
    /// ```rust, no_run
    /// # use dioxus::prelude::*;
    /// #[derive(Clone, Copy)]
    /// struct Logger {
    ///     signal: Signal<i32>,
    /// }
    ///
    /// fn app() -> Element {
    ///     // `use_context_provider` only runs once, so the effect is only created once
    ///     // and will rerun whenever the signal it reads changes.
    ///     use_context_provider(|| {
    ///         let signal = Signal::new(0);
    ///         Effect::new(move || println!("signal is now {signal}"));
    ///         Logger { signal }
    ///     });
    ///     rsx! {}
    /// }
    /// ```
    #[track_caller]
    pub fn new(callback: impl FnMut() + 'static) -> Self {
        Self::new_with_location(callback, std::panic::Location::caller())
    }

    /// Create a new effect with an explicit location for debugging purposes.
    /// This is useful for effects created within closures or macros.
    pub fn new_with_location(
        mut callback: impl FnMut() + 'static,
        location: &'static std::panic::Location<'static>,
    ) -> Self {
        let callback = Callback::new(move |_: ()| callback());

        // Inside the effect, we track any reads so that we can rerun the effect if a value the effect reads changes
        let (rc, mut changed) = ReactiveContext::new_with_origin(location);

        // Deduplicate queued effects
        let effect_queued = Rc::new(Cell::new(false));

        // Spawn a task that will run the effect when:
        // 1) The effect is first created
        // 2) The effect is rerun due to an async read at any time
        // 3) The effect is rerun in the same tick that the component is rerun: we need to wait for the component to rerun before we can run the effect again
        let queue_effect_for_next_render = move || {
            if effect_queued.get() {
                return;
            }
            effect_queued.set(true);
            let effect_queued = effect_queued.clone();
            queue_effect(move || {
                rc.reset_and_run_in(|| callback(()));
                effect_queued.set(false);
            });
        };

        queue_effect_for_next_render();
        spawn(async move {
            loop {
                // Wait for context to change
                let _ = changed.next().await;

                // Run the effect
                queue_effect_for_next_render();
            }
        });
        Effect { rc }
    }

    /// Marks the effect as dirty, causing it to rerun on the next render.
    pub fn mark_dirty(&mut self) {
        self.rc.mark_dirty();
    }
}
