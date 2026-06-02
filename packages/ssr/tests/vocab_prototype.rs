//! Prototype for the "open, composable element-vocabulary" RFC.
//!
//! Two findings drive the representation:
//!
//! 1. Elements are ZST **types** (not modules): you can't `impl` a trait for a module, so a
//!    module can't be extended. A unit struct can, and rsx still resolves the element name +
//!    `TAG_NAME`/`NAME_SPACE` through the same `dioxus_elements::elements::div::TAG_NAME` paths
//!    (`Type::CONST` is valid path syntax). Proven against real `rsx!` + SSR below.
//!
//! 2. Attributes are **methods**, not inherent consts. An inherent const (`div::class`) can't be
//!    added to a foreign type, so a const-based attribute is NOT extensible. A method is: built-in
//!    attributes are inherent/owned-trait methods, extension attributes are methods from another
//!    trait brought into scope, and BOTH are reached through the *same* `el.attr()` path —
//!    indistinguishable at the call site. (This is #712's attribute form; it requires rsx to emit
//!    attribute access as `dioxus_elements::el.attr()` rather than `el::attr.0`.)

use dioxus::prelude::*;

/// (name, namespace, volatile) — the descriptor every attribute method returns.
type AttributeDescription = (&'static str, Option<&'static str>, bool);

/// Hand-written stand-in for what `compose_elements!` would generate.
mod vocab {
    pub mod elements {
        #![allow(non_camel_case_types, non_upper_case_globals, dead_code)]
        use super::super::AttributeDescription;

        /// Web-component element. Underscore-free + lowercase-first (else rsx routes it to the
        /// component path). `slbutton` renders as `sl-button`.
        pub struct slbutton;
        impl slbutton {
            // Element intrinsics stay inherent consts — intrinsic, not extensible, and read by
            // rsx as `elements::slbutton::TAG_NAME` / `slbutton::NAME_SPACE`.
            pub const TAG_NAME: &'static str = "sl-button";
            pub const NAME_SPACE: Option<&'static str> = None;

            // Built-in attributes are METHODS (so extensions can add more via the same path).
            pub fn variant(&self) -> AttributeDescription {
                ("variant", None, false)
            }
            pub fn label(&self) -> AttributeDescription {
                ("aria-label", None, false) // rename: rust `label` -> dom `aria-label`
            }
        }

        /// Namespaced element (MathML).
        pub struct math;
        impl math {
            pub const TAG_NAME: &'static str = "math";
            pub const NAME_SPACE: Option<&'static str> =
                Some("http://www.w3.org/1998/Math/MathML");
            pub fn display(&self) -> AttributeDescription {
                ("display", None, false)
            }
        }
    }

    pub use elements::*;

    #[allow(unused_imports)]
    pub use dioxus::prelude::dioxus_elements::events;
}

use vocab as dioxus_elements;

#[test]
fn element_type_names_resolve_via_real_rsx() {
    // Proves element *names* (ZST types) + TAG_NAME/NAME_SPACE resolve and erase through the
    // real rsx macro. No custom attributes here — today's rsx reads attrs as `el::attr.0`
    // consts; the method-based attribute path (next test) is the proposed rsx change.
    fn app() -> Element {
        rsx! {
            slbutton { "Click me" }
            math { "x" }
        }
    }

    let mut dom = VirtualDom::new(app);
    dom.rebuild(&mut dioxus_core::NoOpMutations);
    assert_eq!(
        dioxus_ssr::render(&dom),
        r#"<sl-button>Click me</sl-button><math>x</math>"#
    );
}

#[test]
fn attributes_are_extensible_through_the_same_path() {
    use vocab::elements::slbutton;

    // A third-party crate adds attributes to an EXISTING element by implementing a trait for it.
    #[allow(non_camel_case_types)]
    trait AriaAttributes {
        fn aria_pressed(&self) -> AttributeDescription;
    }
    impl AriaAttributes for slbutton {
        fn aria_pressed(&self) -> AttributeDescription {
            ("aria-pressed", None, false)
        }
    }

    let el = slbutton;

    // THE SAME PATH: built-in and extension attributes are both `el.attr()`. The call site can't
    // tell which is inherent (built-in) and which is a trait method (extension) in scope — that
    // is exactly what "extensible through the same path" means, and what rsx would emit.
    assert_eq!(el.variant(), ("variant", None, false)); // built-in (inherent)
    assert_eq!(el.label(), ("aria-label", None, false)); // built-in (inherent, renamed)
    assert_eq!(el.aria_pressed(), ("aria-pressed", None, false)); // extension (trait in scope)

    // And the element intrinsics still resolve via the path rsx uses for tag/namespace.
    assert_eq!(slbutton::TAG_NAME, "sl-button");
    assert_eq!(slbutton::NAME_SPACE, None);
}
