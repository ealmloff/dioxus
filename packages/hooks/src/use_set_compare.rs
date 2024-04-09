use std::hash::Hash;

use dioxus_core::prelude::*;
use dioxus_signals::{ReadOnlySignal, SetCompare};

/// Creates a new SetCompare which efficiently tracks when a value changes to check if it is equal to a set of values.
///
/// Generally, you shouldn't need to use this hook. Instead you can use [`crate::use_memo`]. If you have many values that you need to compare to a single value, this hook will change updates from O(n) to O(1) where n is the number of values you are comparing to.
///
/// ```rust
/// use dioxus::prelude::*;
///
/// fn App() -> Element {
///     let mut count = use_signal(|| 0);
///     let compare = use_set_compare(move || count());
///
///     render! {
///         for i in 0..10 {
///             // Child will only re-render when i == count
///             Child { compare, i }
///         }
///         button {
///             // This will only rerender the child with the old and new value of i == count
///             // Because we are using a set compare, this will be O(1) instead of the O(n) performance of a selector
///             onclick: move |_| count += 1,
///             "Increment"
///         }
///     }
/// }
///
/// #[component]
/// fn Child(i: usize, compare: SetCompare<usize>) -> Element {
///     let active = use_set_compare_equal(i, compare);
///     if active() {
///         render! { "Active" }
///     } else {
///         render! { "Inactive" }
///     }
/// }
/// ```
#[must_use]
pub fn use_set_compare<R: Eq + Hash>(f: impl FnMut() -> R + 'static) -> SetCompare<R> {
    use_hook(move || SetCompare::new(f))
}

/// A hook that returns true if the value is equal to the value in the set compare.
#[must_use]
pub fn use_set_compare_equal<R: Eq + Hash>(
    value: R,
    mut compare: SetCompare<R>,
) -> ReadOnlySignal<bool> {
    use_hook(move || compare.equal(value))
}