//! Prototype Stage 2-4 for the open element-vocabulary RFC: prove the *macros* are feasible.
//!
//! - `define_elements!` generates a vocabulary as plain modules (TAG_NAME/NAME_SPACE/attr
//!   consts), a per-vocab `HotReloadingContext`, and a `manifest!` macro exposing its element
//!   idents to the composer.
//! - `compose_elements!` merges N independently-defined vocabularies into ONE in-scope
//!   `dioxus_elements` root: dual element exposure (`root::<el>` + `root::elements::<el>`), a
//!   re-exported event set, a right-nested tuple `HotReloadingContext` (the COMPOSABILITY
//!   layer), and a UNION `CompleteWithBraces` enum built by invoking each vocab's manifest
//!   macro (the macro-generates-macro / CPS mechanism).
//! - Real `rsx!` then resolves elements from BOTH vocabularies in one block and SSR confirms
//!   correct erasure.
//!
//! Two orthogonal layers: composition is the `HotReloadingContext` tuple here; manganis
//! assets (`__ASSETS__`/`SymbolData`) are only the TRANSPORT that carries each vocab's map
//! into the binary so the separately-compiled CLI can build these contexts. See the
//! `vocab_namespace_proto` round-trip test in `packages/manganis/manganis-core/src/ffi.rs`.

use dioxus::prelude::*;
use dioxus_core_types::HotReloadingContext;

// ---------------------------------------------------------------------------------------------
// define_elements!  — declare one vocabulary
// ---------------------------------------------------------------------------------------------
macro_rules! define_elements {
    (
        vocab $vocab:ident ;
        manifest $manifest:ident ;
        $(
            $el:ident => $tag:literal @ $ns:expr ;
            { $( $attr:ident => $aname:literal ),* $(,)? }
        )*
    ) => {
        // Re-enter with a captured dollar token so we can *generate* a macro below.
        define_elements!{ @emit ($) $vocab $manifest
            $( $el => $tag @ $ns ; { $( $attr => $aname ),* } )*
        }
    };

    (@emit ($d:tt) $vocab:ident $manifest:ident
        $( $el:ident => $tag:literal @ $ns:expr ; { $( $attr:ident => $aname:literal ),* } )*
    ) => {
        pub mod $vocab {
            #[allow(non_upper_case_globals, non_camel_case_types, dead_code)]
            pub mod elements {
                $(
                    // Each element is a ZST TYPE with inherent consts (not a module), so it is
                    // open to extension via downstream trait impls. rsx resolution is identical:
                    // `elements::$el::TAG_NAME` / `$el::$attr.0` work the same on a type.
                    pub struct $el;
                    impl $el {
                        pub const TAG_NAME: &'static str = $tag;
                        pub const NAME_SPACE: Option<&'static str> = $ns;
                        $(
                            pub const $attr: (&'static str, Option<&'static str>, bool) =
                                ($aname, None, false);
                        )*
                    }
                )*
            }
            // expose elements at the top level too (root::<el>)
            pub use elements::*;

            // per-vocab hot-reload name map (rust ident -> (dom name, xmlns))
            #[allow(dead_code)]
            pub struct Ctx;
            impl dioxus_core_types::HotReloadingContext for Ctx {
                fn map_element(el: &str) -> Option<(&'static str, Option<&'static str>)> {
                    $( if el == stringify!($el) { return Some(($tag, $ns)); } )*
                    None
                }
                fn map_attribute(
                    el: &str,
                    attr: &str,
                ) -> Option<(&'static str, Option<&'static str>)> {
                    $(
                        if el == stringify!($el) {
                            $( if attr == stringify!($attr) { return Some(($aname, None)); } )*
                        }
                    )*
                    None
                }
            }
        }

        // Generate the manifest macro. CPS: invoke as `$manifest!{ <callback> ; <prefix tokens> }`
        // and it re-invokes `<callback>! { <prefix tokens> @idents [ <el idents> ] }`.
        macro_rules! $manifest {
            ($d cb:path ; $d ( $d pre:tt )* ) => {
                $d cb ! { $d ( $d pre )* @idents [ $( $el )* ] }
            };
        }
    };
}

// ---------------------------------------------------------------------------------------------
// compose_elements!  — merge vocabularies into one `dioxus_elements` root
// ---------------------------------------------------------------------------------------------
macro_rules! compose_elements {
    (
        root $root:ident ;
        vocabs [ $( $vocab:ident => $manifest:ident ),* $(,)? ] ;
    ) => {
        compose_elements!{ @gather
            root $root ;
            vocabs [ $( $vocab ),* ] ;
            remaining [ $( $manifest ),* ] ;
            acc [ ] ;
        }
    };

    // still vocabs to ask: pop the first manifest, invoke it to append its idents
    (@gather
        root $root:ident ;
        vocabs $vocabs:tt ;
        remaining [ $first:ident $(, $rest:ident )* ] ;
        acc [ $( $got:ident )* ] ;
    ) => {
        $first ! {
            compose_elements ;
            @gathered root $root ; vocabs $vocabs ;
            remaining [ $( $rest ),* ] ; acc [ $( $got )* ]
        }
    };

    // manifest appended `@idents [ ... ]`; fold into acc and continue
    (@gathered
        root $root:ident ; vocabs $vocabs:tt ;
        remaining [ $( $rest:ident ),* ] ; acc [ $( $got:ident )* ]
        @idents [ $( $new:ident )* ]
    ) => {
        compose_elements!{ @gather
            root $root ; vocabs $vocabs ;
            remaining [ $( $rest ),* ] ; acc [ $( $got )* $( $new )* ] ;
        }
    };

    // no vocabs left: emit the composed root
    (@gather
        root $root:ident ;
        vocabs [ $( $vocab:ident ),* ] ;
        remaining [ ] ;
        acc [ $( $ident:ident )* ] ;
    ) => {
        pub mod $root {
            pub mod elements {
                $( pub use super::super::$vocab::elements::*; )*

                // The UNION completion enum the rsx completion-hint resolves against.
                #[allow(non_camel_case_types, dead_code)]
                pub mod completions {
                    pub enum CompleteWithBraces { $( $ident {} ),* }
                }
            }
            pub use elements::*;

            // Default event set (HTML). NOTE: making this a macro parameter hit the
            // `$p:path` + `::suffix` concatenation limit, so it's hardcoded for the prototype.
            pub mod events {
                #[allow(unused_imports)]
                pub use dioxus::prelude::dioxus_elements::events::*;
            }

            // right-nested tuple of each vocab's Ctx, terminated by Empty (a type-position
            // macro expansion). `Ctx` lives directly in `$root`, so vocabs are one `super` up.
            pub type Ctx = compose_elements!(@nest $( $vocab )*);
        }
    };

    // build the nested tuple type
    (@nest) => { dioxus_core_types::Empty };
    (@nest $first:ident $( $rest:ident )*) => {
        ( super::$first::Ctx, compose_elements!(@nest $( $rest )*) )
    };
}

// ---------------------------------------------------------------------------------------------
// Two independently-defined vocabularies
// ---------------------------------------------------------------------------------------------
define_elements! {
    vocab corehtml ;
    manifest corehtml_manifest ;
    div => "div" @ None ; { class => "class", id => "id" }
    span => "span" @ None ; { class => "class" }
}

define_elements! {
    vocab widgets ;
    manifest widgets_manifest ;
    // web-component-style (renamed tag) + a renamed attribute
    slbutton => "sl-button" @ None ; { variant => "variant", label => "aria-label", class => "class" }
    // namespaced (MathML)
    math => "math" @ Some("http://www.w3.org/1998/Math/MathML") ; { display => "display" }
}

// ---------------------------------------------------------------------------------------------
// Compose them into one root, then shadow the prelude's `dioxus_elements` with it.
// ---------------------------------------------------------------------------------------------
compose_elements! {
    root app_elements ;
    vocabs [ corehtml => corehtml_manifest, widgets => widgets_manifest ] ;
}

// The generated root is given a unique name (avoids ambiguity with the prelude's glob-imported
// `dioxus_elements`); it's aliased to `dioxus_elements` only at each rsx call site.
use crate::app_elements as composed_root;

#[test]
fn two_vocabularies_compose_and_render() {
    // bring the composed root into rsx scope (explicit import beats the prelude glob)
    use composed_root as dioxus_elements;

    fn app() -> Element {
        rsx! {
            div { class: "wrap",
                slbutton { variant: "primary", label: "Save", "Click" }
                math { display: "block", "x" }
                span { class: "tag", "ok" }
            }
        }
    }

    let mut dom = VirtualDom::new(app);
    dom.rebuild(&mut dioxus_core::NoOpMutations);

    assert_eq!(
        dioxus_ssr::render(&dom),
        r#"<div class="wrap"><sl-button variant="primary" aria-label="Save">Click</sl-button><math display="block">x</math><span class="tag">ok</span></div>"#
    );
}

#[test]
fn composed_hot_reload_context_maps_both_vocabularies() {
    type Ctx = composed_root::Ctx;
    // built-in
    assert_eq!(Ctx::map_element("div"), Some(("div", None)));
    // custom renamed tag
    assert_eq!(Ctx::map_element("slbutton"), Some(("sl-button", None)));
    // renamed attribute
    assert_eq!(
        Ctx::map_attribute("slbutton", "label"),
        Some(("aria-label", None))
    );
    // namespaced element
    assert_eq!(
        Ctx::map_element("math"),
        Some(("math", Some("http://www.w3.org/1998/Math/MathML")))
    );
    // unknown -> None
    assert_eq!(Ctx::map_element("nope"), None);
}

#[test]
fn union_completion_enum_contains_all_vocab_elements() {
    // Compile-time proof the union enum carries variants from BOTH vocabularies.
    #[allow(dead_code)]
    fn _exhaustive(c: composed_root::elements::completions::CompleteWithBraces) {
        use composed_root::elements::completions::CompleteWithBraces::*;
        match c {
            div {} | span {} | slbutton {} | math {} => {}
        }
    }
}
