use crate::BundledAsset;
use const_serialize::{ConstStr, SerializeConst};
use serde::{Deserialize, Serialize};
use std::hash::Hash;

/// Unified symbol data that can represent both assets and permissions
///
/// This enum is used to serialize different types of metadata into the binary
/// using the same `__ASSETS__` symbol prefix. The CBOR format allows for
/// self-describing data, making it easy to add new variants in the future.
///
/// Variant order does NOT matter for CBOR enum serialization - variants are
/// matched by name (string), not by position or tag value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, SerializeConst)]
#[repr(C, u8)]
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
pub enum SymbolData {
    /// An asset that should be bundled with the application
    Asset(BundledAsset),

    /// Android plugin metadata (prebuilt artifacts + Gradle deps)
    AndroidArtifact(AndroidArtifactMetadata),

    /// Swift package metadata (SPM location + product)
    SwiftPackage(SwiftPackageMetadata),
}

/// Platform categories for permission mapping
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SerializeConst)]
pub enum Platform {
    Android,
    Ios,
    Macos,
}

/// Bit flags for supported platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SerializeConst)]
pub struct PlatformFlags(u8);

impl PlatformFlags {
    pub const fn new() -> Self {
        Self(0)
    }
}

impl Default for PlatformFlags {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformFlags {
    pub const fn with_platform(mut self, platform: Platform) -> Self {
        self.0 |= 1 << platform as u8;
        self
    }

    pub const fn supports(&self, platform: Platform) -> bool {
        (self.0 & (1 << platform as u8)) != 0
    }

    pub const fn all() -> Self {
        Self(0b000111) // Android + iOS + macOS
    }

    pub const fn mobile() -> Self {
        Self(0b000011) // Android + iOS
    }
}

/// Platform-specific permission identifiers
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlatformIdentifiers {
    pub android: Option<ConstStr>,
    pub ios: Option<ConstStr>,
    pub macos: Option<ConstStr>,
}

/// Metadata describing an Android plugin artifact (.aar) that must be copied into the host Gradle project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, SerializeConst, Serialize, Deserialize)]
pub struct AndroidArtifactMetadata {
    pub plugin_name: ConstStr,
    pub artifact_path: ConstStr,
    pub gradle_dependencies: ConstStr,
}

impl AndroidArtifactMetadata {
    pub const fn new(
        plugin_name: &'static str,
        artifact_path: &'static str,
        gradle_dependencies: &'static str,
    ) -> Self {
        Self {
            plugin_name: ConstStr::new(plugin_name),
            artifact_path: ConstStr::new(artifact_path),
            gradle_dependencies: ConstStr::new(gradle_dependencies),
        }
    }
}

/// Metadata for a Swift package that needs to be linked into the app (iOS/macOS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, SerializeConst, Serialize, Deserialize)]
pub struct SwiftPackageMetadata {
    pub plugin_name: ConstStr,
    pub package_path: ConstStr,
    pub product: ConstStr,
}

impl SwiftPackageMetadata {
    pub const fn new(
        plugin_name: &'static str,
        package_path: &'static str,
        product: &'static str,
    ) -> Self {
        Self {
            plugin_name: ConstStr::new(plugin_name),
            package_path: ConstStr::new(package_path),
            product: ConstStr::new(product),
        }
    }
}

#[cfg(test)]
mod vocab_namespace_proto {
    //! Prototype for the corrected open-vocabulary hot-reload design: element vocabularies
    //! submit their rust-ident -> (dom name, xml namespace) map into the binary via the same
    //! `__ASSETS__` / `const_serialize` channel that assets and FFI metadata already use, so the
    //! separately-compiled CLI can read them back from the built binary at hot-reload time.
    //!
    //! This proves the *payload format* round-trips on that channel. In the real design this
    //! would become a new `SymbolData` variant emitted via `generate_link_section_inner`.
    use super::*;
    use const_serialize::{ConstVec, deserialize_const, serialize_const};

    /// One element's hot-reload mapping. `namespace` empty = no XML namespace.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, SerializeConst)]
    pub struct ElementNamespaceEntry {
        pub rust_name: ConstStr,
        pub dom_name: ConstStr,
        pub namespace: ConstStr,
    }

    impl ElementNamespaceEntry {
        pub const fn new(rust_name: &str, dom_name: &str, namespace: &str) -> Self {
            Self {
                rust_name: ConstStr::new(rust_name),
                dom_name: ConstStr::new(dom_name),
                namespace: ConstStr::new(namespace),
            }
        }
    }

    #[test]
    fn namespace_entry_round_trips_through_const_serialize() {
        // a renamed + namespaced element, exactly the case the string-fallback gets wrong
        const ENTRY: ElementNamespaceEntry =
            ElementNamespaceEntry::new("mathroot", "msqrt", "http://www.w3.org/1998/Math/MathML");

        // serialize at const time (as the link-section macro would)
        const BUF: ConstVec<u8> = serialize_const(&ENTRY, ConstVec::new());
        let bytes = BUF.as_ref();

        // deserialize as the CLI would after reading the __ASSETS__ symbol from the binary
        let (_rest, decoded) =
            deserialize_const!(ElementNamespaceEntry, bytes).expect("decode namespace entry");

        assert_eq!(decoded, ENTRY);
        assert_eq!(decoded.rust_name.as_str(), "mathroot");
        assert_eq!(decoded.dom_name.as_str(), "msqrt");
        assert_eq!(decoded.namespace.as_str(), "http://www.w3.org/1998/Math/MathML");
    }

    #[test]
    fn unnamespaced_renamed_element_round_trips() {
        const ENTRY: ElementNamespaceEntry =
            ElementNamespaceEntry::new("slbutton", "sl-button", "");
        const BUF: ConstVec<u8> = serialize_const(&ENTRY, ConstVec::new());
        let (_rest, decoded) =
            deserialize_const!(ElementNamespaceEntry, BUF.as_ref()).expect("decode");
        assert_eq!(decoded.dom_name.as_str(), "sl-button");
        assert!(decoded.namespace.as_str().is_empty()); // empty => no namespace
    }
}
