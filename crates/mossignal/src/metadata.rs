//! Owned descriptive metadata for authored semantic subjects.
//!
//! Metadata supplies human-readable names, descriptions, hierarchy, source
//! correlation, and tags. It does not define stable subject identity and does
//! not affect signal behavior. All text is owned and retained exactly without
//! trimming, case folding, Unicode normalization, or line-ending conversion.

/// An owned descriptive annotation for an authored semantic subject.
///
/// Every field is presentation metadata rather than stable identity or signal
/// behavior. Absence remains distinct from an explicitly supplied empty value.
/// Tags retain caller-supplied order, duplicates, and empty strings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticMeta {
    /// An optional human-readable name retained exactly as supplied.
    pub name: Option<String>,
    /// Optional explanatory text retained exactly as supplied.
    pub description: Option<String>,
    /// An optional ordered descriptive hierarchy.
    pub path: Option<DiagnosticPath>,
    /// Optional caller-supplied authored-source correlation.
    pub origin: Option<OriginRef>,
    /// Caller-defined descriptive tags with exact sequence semantics.
    pub tags: Vec<String>,
}

/// An owned ordered descriptive hierarchy.
///
/// A diagnostic path is not an operating-system path, URI, graph path,
/// ownership relation, or stable identity. Segments are never parsed or
/// normalized: order, repetition, empty strings, separators, and text bytes
/// are retained exactly.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticPath {
    segments: Vec<String>,
}

impl DiagnosticPath {
    /// Creates an empty diagnostic path.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Creates a path by converting each supplied segment into owned text.
    ///
    /// Iterator order and every segment's exact contents are preserved.
    #[must_use]
    pub fn from_segments<I, S>(segments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            segments: segments.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns the complete ordered segment sequence.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Returns the number of stored segments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// Returns whether this path contains no segments.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Returns ownership of the complete ordered segment vector.
    #[must_use]
    pub fn into_segments(self) -> Vec<String> {
        self.segments
    }
}

/// Owned caller-supplied correlation to an authored source.
///
/// An origin can help tooling relate a subject to an editor object, imported
/// asset, source document, generated definition, or other caller-defined
/// source label. The core library does not interpret or resolve it. An origin
/// is not runtime provenance and does not describe causal execution history.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginRef {
    /// Arbitrary caller-defined source-correlation text.
    Text(String),
    /// A source label and opaque caller-defined coordinates.
    SourceLocation {
        /// Caller-defined source text retained exactly as supplied.
        source: String,
        /// A caller-defined line coordinate with no imposed indexing convention.
        line: u32,
        /// A caller-defined column coordinate with no imposed indexing convention.
        column: u32,
    },
}

impl OriginRef {
    /// Creates an origin from arbitrary borrowed or owned source-correlation text.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// Creates an origin from source text and opaque caller-defined coordinates.
    ///
    /// Every `u32` line and column value is accepted, including zero and
    /// `u32::MAX`. The source is not interpreted as a filesystem path or URI.
    #[must_use]
    pub fn source_location(source: impl Into<String>, line: u32, column: u32) -> Self {
        Self::SourceLocation {
            source: source.into(),
            line,
            column,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn populated_meta() -> DiagnosticMeta {
        DiagnosticMeta {
            name: Some(String::from("gate controller")),
            description: Some(String::from("first line\nsecond line")),
            path: Some(DiagnosticPath::from_segments([
                "west wing",
                "control circuit",
            ])),
            origin: Some(OriginRef::source_location("castle.logic", 4, 7)),
            tags: vec![
                String::from("control"),
                String::from("critical"),
                String::from("control"),
            ],
        }
    }

    #[test]
    fn default_metadata_is_completely_empty() {
        let metadata = DiagnosticMeta::default();

        assert_eq!(metadata.name, None);
        assert_eq!(metadata.description, None);
        assert_eq!(metadata.path, None);
        assert_eq!(metadata.origin, None);
        assert!(metadata.tags.is_empty());
        assert_eq!(metadata, DiagnosticMeta::default());
        assert!(!format!("{metadata:?}").is_empty());
    }

    #[test]
    fn metadata_equality_includes_every_field_and_preserves_absence() {
        let baseline = populated_meta();
        assert_eq!(baseline, baseline.clone());

        let mut changed = baseline.clone();
        changed.name = Some(String::from("other"));
        assert_ne!(baseline, changed);

        let mut changed = baseline.clone();
        changed.description = Some(String::from("other"));
        assert_ne!(baseline, changed);

        let mut changed = baseline.clone();
        changed.path = Some(DiagnosticPath::from_segments(["other"]));
        assert_ne!(baseline, changed);

        let mut changed = baseline.clone();
        changed.origin = Some(OriginRef::text("other"));
        assert_ne!(baseline, changed);

        let mut changed = baseline.clone();
        changed.tags.push(String::from("other"));
        assert_ne!(baseline, changed);

        let empty = DiagnosticMeta::default();
        assert_ne!(
            empty,
            DiagnosticMeta {
                name: Some(String::new()),
                ..DiagnosticMeta::default()
            }
        );
        assert_ne!(
            empty,
            DiagnosticMeta {
                description: Some(String::new()),
                ..DiagnosticMeta::default()
            }
        );
        assert_ne!(
            empty,
            DiagnosticMeta {
                path: Some(DiagnosticPath::default()),
                ..DiagnosticMeta::default()
            }
        );
        assert_ne!(
            empty,
            DiagnosticMeta {
                origin: Some(OriginRef::text("")),
                ..DiagnosticMeta::default()
            }
        );
        assert_ne!(
            empty,
            DiagnosticMeta {
                tags: vec![String::new()],
                ..DiagnosticMeta::default()
            }
        );
    }

    #[test]
    fn diagnostic_paths_preserve_owned_ordered_segments_exactly() {
        const EMPTY: DiagnosticPath = DiagnosticPath::new();
        assert!(EMPTY.is_empty());
        assert_eq!(EMPTY.len(), 0);
        assert!(DiagnosticPath::default().is_empty());

        let borrowed = ["a", "", "b", "a", "a/b", ".", "..", "x::y"];
        let path = DiagnosticPath::from_segments(borrowed);
        assert_eq!(path.len(), borrowed.len());
        assert!(!path.is_empty());
        assert_eq!(
            path.segments(),
            ["a", "", "b", "a", "a/b", ".", "..", "x::y"]
        );
        assert_eq!(
            path.clone().into_segments(),
            borrowed.map(String::from).to_vec()
        );

        let owned = vec![String::from("module"), String::from("inner node")];
        let path = DiagnosticPath::from_segments(owned.clone());
        assert_eq!(path.into_segments(), owned);

        assert_ne!(
            DiagnosticPath::from_segments(["a", "b"]),
            DiagnosticPath::from_segments(["b", "a"])
        );
        assert_ne!(
            DiagnosticPath::from_segments(["a/b"]),
            DiagnosticPath::from_segments(["a", "b"])
        );
        assert_eq!(
            DiagnosticPath::from_segments(["a", "a"]).segments(),
            ["a", "a"]
        );
        assert_eq!(
            DiagnosticPath::from_segments(["a", "", "b"]).segments(),
            ["a", "", "b"]
        );
    }

    #[test]
    fn origin_variants_retain_exact_text_and_opaque_coordinates() {
        let text = OriginRef::text(" generated::source ");
        assert_eq!(text, OriginRef::Text(String::from(" generated::source ")));

        let zero = OriginRef::source_location("source", 0, 0);
        assert_eq!(
            zero,
            OriginRef::SourceLocation {
                source: String::from("source"),
                line: 0,
                column: 0,
            }
        );
        let maximum = OriginRef::source_location("source", u32::MAX, u32::MAX);
        assert_eq!(
            maximum,
            OriginRef::SourceLocation {
                source: String::from("source"),
                line: u32::MAX,
                column: u32::MAX,
            }
        );

        let location = OriginRef::source_location("source", 4, 7);
        assert_ne!(location, OriginRef::source_location("other", 4, 7));
        assert_ne!(location, OriginRef::source_location("source", 5, 7));
        assert_ne!(location, OriginRef::source_location("source", 4, 8));
        assert_ne!(location, OriginRef::text("source"));
        assert!(!format!("{location:?}").is_empty());
    }

    #[test]
    fn tag_sequences_preserve_order_duplicates_empty_values_and_clone() {
        let metadata = DiagnosticMeta {
            tags: vec![
                String::from("debug"),
                String::new(),
                String::from("input"),
                String::from("debug"),
            ],
            ..DiagnosticMeta::default()
        };
        assert_eq!(
            metadata.tags,
            [
                String::from("debug"),
                String::new(),
                String::from("input"),
                String::from("debug")
            ]
        );
        assert_eq!(metadata.tags, metadata.clone().tags);

        let reordered = DiagnosticMeta {
            tags: vec![
                String::from("input"),
                String::new(),
                String::from("debug"),
                String::from("debug"),
            ],
            ..DiagnosticMeta::default()
        };
        assert_ne!(metadata, reordered);

        let fewer_duplicates = DiagnosticMeta {
            tags: vec![String::from("debug"), String::new(), String::from("input")],
            ..DiagnosticMeta::default()
        };
        assert_ne!(metadata, fewer_duplicates);
    }

    #[test]
    fn cloning_metadata_creates_behaviorally_independent_ownership() {
        let original = populated_meta();
        let mut cloned = original.clone();
        assert_eq!(original, cloned);

        if let Some(name) = &mut cloned.name {
            name.push_str(" changed");
        }
        if let Some(description) = &mut cloned.description {
            description.push_str("\nthird line");
        }
        let cloned_path = match cloned.path.take() {
            Some(path) => path,
            None => panic!("populated metadata must contain a path"),
        };
        let mut cloned_segments = cloned_path.into_segments();
        cloned_segments.push(String::from("detached segment"));
        cloned.path = Some(DiagnosticPath::from_segments(cloned_segments));
        cloned.origin = Some(OriginRef::text("replacement origin"));
        cloned.tags.push(String::from("detached tag"));

        assert_eq!(original, populated_meta());
        assert_ne!(original, cloned);
        let original_path = match &original.path {
            Some(path) => path,
            None => panic!("populated metadata must contain a path"),
        };
        assert_eq!(
            original_path.segments(),
            [String::from("west wing"), String::from("control circuit")]
        );
        assert_eq!(
            original.tags,
            [
                String::from("control"),
                String::from("critical"),
                String::from("control")
            ]
        );
    }

    #[test]
    fn borrowed_local_inputs_become_independently_owned() {
        let path = {
            let first = String::from("outer");
            let second = String::from("inner");
            DiagnosticPath::from_segments([first.as_str(), second.as_str()])
        };
        assert_eq!(path.segments(), ["outer", "inner"]);

        let text_origin = {
            let text = String::from("generated definition");
            OriginRef::text(text.as_str())
        };
        assert_eq!(
            text_origin,
            OriginRef::Text(String::from("generated definition"))
        );

        let location_origin = {
            let source = String::from("generated.definition");
            OriginRef::source_location(source.as_str(), 4, 7)
        };
        assert_eq!(
            location_origin,
            OriginRef::SourceLocation {
                source: String::from("generated.definition"),
                line: 4,
                column: 7,
            }
        );
    }

    #[test]
    fn metadata_text_is_preserved_byte_for_byte_through_clone() {
        let composed = "caf\u{e9}";
        let decomposed = "cafe\u{301}";
        assert_ne!(composed.as_bytes(), decomposed.as_bytes());

        let metadata = DiagnosticMeta {
            name: Some(String::from(" leading and trailing \r\n")),
            description: Some(String::from(
                "line one\nline two\r\n\u{96ea}\u{306e}\u{9580}",
            )),
            path: Some(DiagnosticPath::from_segments([
                "",
                " repeated::separator ",
                composed,
                decomposed,
            ])),
            origin: Some(OriginRef::text("\n origin \r\n")),
            tags: vec![String::new(), String::from(" t\u{e4}g ")],
        };
        let cloned = metadata.clone();

        assert_eq!(metadata, cloned);
        assert_eq!(cloned.name.as_deref(), Some(" leading and trailing \r\n"));
        assert_eq!(
            cloned.description.as_deref(),
            Some("line one\nline two\r\n\u{96ea}\u{306e}\u{9580}")
        );
        let cloned_path = match &cloned.path {
            Some(path) => path,
            None => panic!("text-preservation metadata must contain a path"),
        };
        assert_eq!(
            cloned_path.segments(),
            [
                String::new(),
                String::from(" repeated::separator "),
                String::from(composed),
                String::from(decomposed)
            ]
        );
        assert_eq!(cloned.origin, Some(OriginRef::text("\n origin \r\n")));
        assert_eq!(cloned.tags, [String::new(), String::from(" t\u{e4}g ")]);
    }
}
