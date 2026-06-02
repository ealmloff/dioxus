pub trait HotReloadingContext {
    fn map_attribute(
        element_name_rust: &str,
        attribute_name_rust: &str,
    ) -> Option<(&'static str, Option<&'static str>)>;
    fn map_element(element_name_rust: &str) -> Option<(&'static str, Option<&'static str>)>;
}

pub struct Empty;

impl HotReloadingContext for Empty {
    fn map_attribute(_: &str, _: &str) -> Option<(&'static str, Option<&'static str>)> {
        None
    }

    fn map_element(_: &str) -> Option<(&'static str, Option<&'static str>)> {
        None
    }
}

/// Compose two contexts with first-match-wins precedence: `A` is consulted first, falling
/// back to `B`. Fully monomorphized (zero runtime cost), associative, so arbitrary chains
/// compose as right-nested tuples `(A, (B, (C, Empty)))` with `Empty` as the identity. This
/// is the composability primitive for an open, multi-vocabulary system: each vocabulary
/// contributes a `HotReloadingContext` and they merge here with explicit precedence.
impl<A: HotReloadingContext, B: HotReloadingContext> HotReloadingContext for (A, B) {
    fn map_attribute(
        element_name_rust: &str,
        attribute_name_rust: &str,
    ) -> Option<(&'static str, Option<&'static str>)> {
        A::map_attribute(element_name_rust, attribute_name_rust)
            .or_else(|| B::map_attribute(element_name_rust, attribute_name_rust))
    }

    fn map_element(element_name_rust: &str) -> Option<(&'static str, Option<&'static str>)> {
        A::map_element(element_name_rust).or_else(|| B::map_element(element_name_rust))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Html;
    impl HotReloadingContext for Html {
        fn map_attribute(el: &str, attr: &str) -> Option<(&'static str, Option<&'static str>)> {
            match (el, attr) {
                ("div", "class") => Some(("class", None)),
                _ => None,
            }
        }
        fn map_element(el: &str) -> Option<(&'static str, Option<&'static str>)> {
            match el {
                "div" => Some(("div", None)),
                _ => None,
            }
        }
    }

    struct Mathml;
    impl HotReloadingContext for Mathml {
        fn map_attribute(el: &str, attr: &str) -> Option<(&'static str, Option<&'static str>)> {
            match (el, attr) {
                // a renamed attribute under a custom element
                ("mathroot", "display_style") => Some(("displaystyle", None)),
                _ => None,
            }
        }
        fn map_element(el: &str) -> Option<(&'static str, Option<&'static str>)> {
            match el {
                // a renamed + namespaced element
                "mathroot" => Some(("msqrt", Some("http://www.w3.org/1998/Math/MathML"))),
                _ => None,
            }
        }
    }

    type Composed = (Html, (Mathml, Empty));

    #[test]
    fn composes_with_first_match_wins() {
        // built-in still resolves
        assert_eq!(Composed::map_element("div"), Some(("div", None)));
        assert_eq!(Composed::map_attribute("div", "class"), Some(("class", None)));

        // custom vocabulary resolves: renamed + namespaced element, renamed attr
        assert_eq!(
            Composed::map_element("mathroot"),
            Some(("msqrt", Some("http://www.w3.org/1998/Math/MathML")))
        );
        assert_eq!(
            Composed::map_attribute("mathroot", "display_style"),
            Some(("displaystyle", None))
        );

        // unknown falls through the whole chain to None (-> hot-reload string fallback / rebuild)
        assert_eq!(Composed::map_element("unknowntag"), None);
    }

    #[test]
    fn precedence_is_first_match() {
        struct Override;
        impl HotReloadingContext for Override {
            fn map_attribute(_: &str, _: &str) -> Option<(&'static str, Option<&'static str>)> {
                None
            }
            fn map_element(el: &str) -> Option<(&'static str, Option<&'static str>)> {
                match el {
                    "div" => Some(("OVERRIDDEN", None)),
                    _ => None,
                }
            }
        }
        // Html is first, so it wins for "div" even though Override also matches.
        assert_eq!(<(Html, Override)>::map_element("div"), Some(("div", None)));
        // Override first -> it wins.
        assert_eq!(
            <(Override, Html)>::map_element("div"),
            Some(("OVERRIDDEN", None))
        );
    }
}
