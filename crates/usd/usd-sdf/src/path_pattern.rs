//! Path pattern matching for SDF paths.
//!
//! Port of pxr/usd/sdf/pathPattern.h
//!
//! Path patterns consist of an SdfPath prefix followed by components
//! that may contain wildcards and optional predicate expressions.
//!
//! # Examples
//!
//! - `/World` - exact path match
//! - `/World/*` - any direct child of /World
//! - `/World//` - any descendant of /World
//! - `/World/Char*` - children starting with "Char"
//! - `/World//{active}` - descendants matching predicate

use crate::{Path, PredicateExpression};
use std::fmt;

/// A component in a path pattern after the prefix.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathPatternComponent {
    /// The text of this component (may contain wildcards).
    pub text: String,
    /// Index into the pattern's predicate expression list, or -1 if none.
    pub predicate_index: i32,
    /// True if this is a literal (no wildcards).
    pub is_literal: bool,
}

impl PathPatternComponent {
    /// Creates a new literal component.
    pub fn literal(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            predicate_index: -1,
            is_literal: true,
        }
    }

    /// Creates a new wildcard component.
    pub fn wildcard(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            predicate_index: -1,
            is_literal: false,
        }
    }

    /// Creates a stretch component (//), matching arbitrary hierarchy depth.
    pub fn stretch() -> Self {
        Self {
            text: String::new(),
            predicate_index: -1,
            is_literal: false,
        }
    }

    /// Creates a component with a predicate.
    pub fn with_predicate(text: impl Into<String>, predicate_index: i32, is_literal: bool) -> Self {
        Self {
            text: text.into(),
            predicate_index,
            is_literal,
        }
    }

    /// Returns true if this is a stretch component (//).
    pub fn is_stretch(&self) -> bool {
        self.predicate_index == -1 && self.text.is_empty()
    }

    /// Returns true if this component has a predicate.
    pub fn has_predicate(&self) -> bool {
        self.predicate_index >= 0
    }

    /// Returns true if this component contains wildcards.
    pub fn has_wildcards(&self) -> bool {
        !self.is_literal && !self.is_stretch()
    }
}

impl Default for PathPatternComponent {
    fn default() -> Self {
        Self::stretch()
    }
}

/// A path pattern for matching SDF paths.
///
/// Patterns consist of a prefix path followed by optional components
/// with wildcards or predicates.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SdfPathPattern {
    /// The non-speculative prefix (path without wildcards/predicates).
    prefix: Path,
    /// Components after the prefix.
    components: Vec<PathPatternComponent>,
    /// Predicate expressions referenced by components.
    predicates: Vec<PredicateExpression>,
    /// Whether this pattern matches properties (vs prims).
    is_property: bool,
}

impl SdfPathPattern {
    /// Creates an empty pattern that matches nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a pattern from a prefix path.
    pub fn from_prefix(prefix: Path) -> Self {
        Self {
            prefix,
            components: Vec::new(),
            predicates: Vec::new(),
            is_property: false,
        }
    }

    /// Returns the pattern "//" which matches all absolute paths.
    pub fn everything() -> Self {
        Self {
            prefix: Path::absolute_root(),
            components: vec![PathPatternComponent::stretch()],
            predicates: Vec::new(),
            is_property: false,
        }
    }

    /// Returns the pattern ".//" which matches all paths descendant to anchor.
    pub fn every_descendant() -> Self {
        Self {
            prefix: Path::reflexive_relative(),
            components: vec![PathPatternComponent::stretch()],
            predicates: Vec::new(),
            is_property: false,
        }
    }

    /// Returns a pattern that matches nothing.
    pub fn nothing() -> Self {
        Self::new()
    }

    /// Returns the prefix path.
    pub fn get_prefix(&self) -> &Path {
        &self.prefix
    }

    /// Sets the prefix path.
    pub fn set_prefix(&mut self, prefix: Path) -> &mut Self {
        self.prefix = prefix;
        self
    }

    /// Returns the components.
    pub fn get_components(&self) -> &[PathPatternComponent] {
        &self.components
    }

    /// Returns the predicate expressions.
    pub fn get_predicates(&self) -> &[PredicateExpression] {
        &self.predicates
    }

    /// Returns true if this pattern is empty (matches nothing).
    pub fn is_empty(&self) -> bool {
        self.prefix.is_empty() && self.components.is_empty()
    }

    /// Returns true if this pattern is a property pattern.
    pub fn is_property(&self) -> bool {
        self.is_property
    }

    /// Returns true if this pattern's prefix is absolute.
    pub fn is_absolute(&self) -> bool {
        self.prefix.is_absolute_path()
    }

    /// Returns true if the pattern starts with a stretch (//...).
    pub fn has_leading_stretch(&self) -> bool {
        self.prefix.is_absolute_root_path()
            && !self.components.is_empty()
            && self.components[0].is_stretch()
    }

    /// Returns true if the pattern ends with a stretch (...//).
    pub fn has_trailing_stretch(&self) -> bool {
        !self.components.is_empty() && self.components.last().expect("not empty").is_stretch()
    }

    /// Checks if text contains wildcard characters.
    pub fn contains_wildcards(text: &str) -> bool {
        text.contains('*') || text.contains('?')
    }

    /// Returns true if this pattern contains any wildcards or predicates.
    pub fn has_wildcards_or_predicates(&self) -> bool {
        self.components
            .iter()
            .any(|c| c.has_wildcards() || c.has_predicate())
    }

    /// Returns true if it's valid to append a child component.
    pub fn can_append_child(&self, text: &str, reason: Option<&mut String>) -> bool {
        self.can_append_child_with_predicate(text, None, reason)
    }

    /// Returns true if it's valid to append a child with predicate.
    pub fn can_append_child_with_predicate(
        &self,
        text: &str,
        _pred: Option<&PredicateExpression>,
        reason: Option<&mut String>,
    ) -> bool {
        if self.is_property {
            if let Some(r) = reason {
                *r = "Cannot append child to property pattern".to_string();
            }
            return false;
        }
        if text.is_empty() {
            if let Some(r) = reason {
                *r = "Cannot append empty child name".to_string();
            }
            return false;
        }
        true
    }

    /// Appends a prim child component.
    pub fn append_child(&mut self, text: impl Into<String>) -> &mut Self {
        self.append_child_with_predicate(text, None)
    }

    /// Appends a prim child component with optional predicate.
    pub fn append_child_with_predicate(
        &mut self,
        text: impl Into<String>,
        pred: Option<PredicateExpression>,
    ) -> &mut Self {
        let text = text.into();
        let has_wildcards = Self::contains_wildcards(&text);

        // If no wildcards, predicates, or existing components, extend prefix
        if !has_wildcards && pred.is_none() && self.components.is_empty() {
            if let Some(new_prefix) = self.prefix.append_child(&text) {
                self.prefix = new_prefix;
                return self;
            }
        }

        // Otherwise add as component
        let predicate_index = if let Some(p) = pred {
            let idx = self.predicates.len() as i32;
            self.predicates.push(p);
            idx
        } else {
            -1
        };

        self.components.push(PathPatternComponent {
            text,
            predicate_index,
            is_literal: !has_wildcards,
        });
        self
    }

    /// Returns true if it's valid to append a property component.
    pub fn can_append_property(&self, text: &str, reason: Option<&mut String>) -> bool {
        self.can_append_property_with_predicate(text, None, reason)
    }

    /// Returns true if it's valid to append a property with predicate.
    pub fn can_append_property_with_predicate(
        &self,
        text: &str,
        _pred: Option<&PredicateExpression>,
        reason: Option<&mut String>,
    ) -> bool {
        if self.is_property {
            if let Some(r) = reason {
                *r = "Already a property pattern".to_string();
            }
            return false;
        }
        if text.is_empty() {
            if let Some(r) = reason {
                *r = "Cannot append empty property name".to_string();
            }
            return false;
        }
        true
    }

    /// Appends a property component.
    pub fn append_property(&mut self, text: impl Into<String>) -> &mut Self {
        self.append_property_with_predicate(text, None)
    }

    /// Appends a property component with optional predicate.
    pub fn append_property_with_predicate(
        &mut self,
        text: impl Into<String>,
        pred: Option<PredicateExpression>,
    ) -> &mut Self {
        let text = text.into();
        let has_wildcards = Self::contains_wildcards(&text);

        // If no wildcards, predicates, or existing components, extend prefix
        if !has_wildcards && pred.is_none() && self.components.is_empty() {
            if let Some(new_prefix) = self.prefix.append_property(&text) {
                self.prefix = new_prefix;
                self.is_property = true;
                return self;
            }
        }

        // Otherwise add as component
        let predicate_index = if let Some(p) = pred {
            let idx = self.predicates.len() as i32;
            self.predicates.push(p);
            idx
        } else {
            -1
        };

        self.components.push(PathPatternComponent {
            text,
            predicate_index,
            is_literal: !has_wildcards,
        });
        self.is_property = true;
        self
    }

    /// Appends a stretch component (//).
    pub fn append_stretch(&mut self) -> &mut Self {
        // Can't append stretch if already ends with stretch or is property
        if self.has_trailing_stretch() || self.is_property {
            return self;
        }
        self.components.push(PathPatternComponent::stretch());
        self
    }

    /// Removes trailing stretch if present.
    pub fn remove_trailing_stretch(&mut self) -> &mut Self {
        if self.has_trailing_stretch() {
            self.components.pop();
        }
        self
    }

    /// Replaces path prefix.
    pub fn replace_prefix(&self, old_prefix: &Path, new_prefix: &Path) -> Self {
        let mut result = self.clone();
        if let Some(new_path) = result.prefix.replace_prefix(old_prefix, new_prefix) {
            result.prefix = new_path;
        }
        result
    }

    /// Makes relative paths absolute using anchor.
    pub fn make_absolute(&self, anchor: &Path) -> Self {
        let mut result = self.clone();
        if !result.prefix.is_absolute_path() {
            if let Some(abs_path) = result.prefix.make_absolute(anchor) {
                result.prefix = abs_path;
            }
        }
        result
    }

    /// Returns true if this pattern matches the given path.
    ///
    /// This is a simplified matching algorithm that handles:
    /// - Exact prefix matching
    /// - Stretch components (//)
    /// - Simple wildcards (* and ?)
    pub fn matches(&self, path: &Path) -> bool {
        // Empty pattern matches nothing
        if self.is_empty() {
            return false;
        }

        // No components - exact prefix match required
        if self.components.is_empty() {
            return path == &self.prefix;
        }

        // Must have the prefix
        if !path.has_prefix(&self.prefix) {
            return false;
        }

        // Single stretch component - matches any descendant
        if self.components.len() == 1 && self.components[0].is_stretch() {
            return true;
        }

        // Get the path suffix after the prefix
        let path_str = path.as_str();
        let prefix_str = self.prefix.as_str();
        let suffix = if prefix_str == "/" {
            &path_str[1..]
        } else {
            path_str.strip_prefix(prefix_str).unwrap_or("")
        };
        let suffix = suffix.trim_start_matches('/');

        // Match components against suffix
        self.match_components(suffix, 0)
    }

    /// Recursive component matching.
    fn match_components(&self, remaining: &str, comp_idx: usize) -> bool {
        if comp_idx >= self.components.len() {
            return remaining.is_empty();
        }

        let comp = &self.components[comp_idx];

        if comp.is_stretch() {
            // Stretch matches zero or more path segments
            if comp_idx + 1 >= self.components.len() {
                // Trailing stretch - matches everything
                return true;
            }

            // Try matching rest at each position
            let mut pos = remaining;
            loop {
                if self.match_components(pos, comp_idx + 1) {
                    return true;
                }
                // Skip to next segment
                match pos.find('/') {
                    Some(idx) => pos = &pos[idx + 1..],
                    None => break,
                }
            }
            // Also try with empty remaining
            self.match_components("", comp_idx + 1)
        } else {
            // Regular component - must match next segment
            let (segment, rest) = match remaining.find('/') {
                Some(idx) => (&remaining[..idx], &remaining[idx + 1..]),
                None => (remaining, ""),
            };

            if segment.is_empty() {
                return false;
            }

            if self.match_segment(segment, comp) {
                self.match_components(rest, comp_idx + 1)
            } else {
                false
            }
        }
    }

    /// Matches a path segment against a component.
    fn match_segment(&self, segment: &str, comp: &PathPatternComponent) -> bool {
        if comp.is_literal {
            segment == comp.text
        } else {
            // Wildcard matching
            self.match_wildcard(segment, &comp.text)
        }
    }

    /// Simple wildcard matching (* and ?).
    fn match_wildcard(&self, text: &str, pattern: &str) -> bool {
        let mut t_chars = text.chars().peekable();
        let mut p_chars = pattern.chars().peekable();

        while let Some(p) = p_chars.next() {
            match p {
                '*' => {
                    // Skip consecutive *
                    while p_chars.peek() == Some(&'*') {
                        p_chars.next();
                    }

                    // * at end matches everything
                    if p_chars.peek().is_none() {
                        return true;
                    }

                    // Try matching rest at each position
                    let rest_pattern: String = p_chars.collect();
                    let mut remaining: String = t_chars.collect();
                    while !remaining.is_empty() {
                        if self.match_wildcard(&remaining, &rest_pattern) {
                            return true;
                        }
                        remaining = remaining.chars().skip(1).collect();
                    }
                    return self.match_wildcard("", &rest_pattern);
                }
                '?' => {
                    // ? matches exactly one character
                    if t_chars.next().is_none() {
                        return false;
                    }
                }
                c => {
                    // Literal character
                    if t_chars.next() != Some(c) {
                        return false;
                    }
                }
            }
        }

        // Pattern exhausted - text must also be exhausted
        t_chars.next().is_none()
    }
}

impl From<Path> for SdfPathPattern {
    fn from(path: Path) -> Self {
        Self::from_prefix(path)
    }
}

impl fmt::Display for SdfPathPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.prefix.as_str())?;
        for comp in &self.components {
            if comp.is_stretch() {
                write!(f, "//")?;
            } else {
                write!(f, "/{}", comp.text)?;
                if comp.predicate_index >= 0 {
                    write!(f, "{{...}}")?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_pattern() {
        let p = SdfPathPattern::new();
        assert!(p.is_empty());
    }

    #[test]
    fn test_everything() {
        let p = SdfPathPattern::everything();
        assert!(p.has_leading_stretch());
        assert!(p.has_trailing_stretch());
    }

    #[test]
    fn test_from_prefix() {
        let p = SdfPathPattern::from_prefix(Path::from_string("/World").unwrap());
        assert!(!p.is_empty());
        assert!(p.is_absolute());
    }

    #[test]
    fn test_append_child() {
        let mut p = SdfPathPattern::from_prefix(Path::absolute_root());
        p.append_child("World");
        assert_eq!(p.get_prefix().as_str(), "/World");
    }

    #[test]
    fn test_append_wildcard() {
        let mut p = SdfPathPattern::from_prefix(Path::from_string("/World").unwrap());
        p.append_child("Char*");
        assert_eq!(p.get_components().len(), 1);
        assert!(!p.get_components()[0].is_literal);
    }

    #[test]
    fn test_match_exact() {
        let p = SdfPathPattern::from_prefix(Path::from_string("/World/Cube").unwrap());
        assert!(p.matches(&Path::from_string("/World/Cube").unwrap()));
        assert!(!p.matches(&Path::from_string("/World").unwrap()));
        assert!(!p.matches(&Path::from_string("/World/Cube/Child").unwrap()));
    }

    #[test]
    fn test_match_stretch() {
        let p = SdfPathPattern::everything();
        assert!(p.matches(&Path::from_string("/World").unwrap()));
        assert!(p.matches(&Path::from_string("/World/Cube").unwrap()));
        assert!(p.matches(&Path::from_string("/A/B/C/D").unwrap()));
    }

    #[test]
    fn test_match_wildcard() {
        let mut p = SdfPathPattern::from_prefix(Path::from_string("/World").unwrap());
        p.append_child("Char*");
        assert!(p.matches(&Path::from_string("/World/Character").unwrap()));
        assert!(p.matches(&Path::from_string("/World/Char").unwrap()));
        assert!(p.matches(&Path::from_string("/World/Char123").unwrap()));
        assert!(!p.matches(&Path::from_string("/World/Other").unwrap()));
    }

    #[test]
    fn test_contains_wildcards() {
        assert!(SdfPathPattern::contains_wildcards("foo*"));
        assert!(SdfPathPattern::contains_wildcards("foo?bar"));
        assert!(SdfPathPattern::contains_wildcards("*"));
        assert!(!SdfPathPattern::contains_wildcards("foo"));
        assert!(!SdfPathPattern::contains_wildcards(""));
    }
}
