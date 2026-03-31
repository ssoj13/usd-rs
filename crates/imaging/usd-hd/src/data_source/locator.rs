//! Data source locator - paths for addressing nested data.

use std::fmt;
use std::hash::{Hash, Hasher};
use usd_tf::Token;

/// A path for identifying locations within nested container data sources.
///
/// A data source locator is a sequence of tokens that identifies a specific
/// data source within a nested container hierarchy, similar to a file path.
///
/// # Examples
///
/// ```
/// use usd_hd::data_source::*;
/// use usd_tf::Token;
///
/// // Create a locator for "primvars.points"
/// let locator = HdDataSourceLocator::new(&[
///     Token::new("primvars"),
///     Token::new("points"),
/// ]);
///
/// // Append to existing locator
/// let extended = locator.append(&Token::new("value"));
/// // Now points to "primvars.points.value"
///
/// // Check prefixes
/// let prefix = HdDataSourceLocator::new(&[Token::new("primvars")]);
/// assert!(locator.has_prefix(&prefix));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HdDataSourceLocator {
    /// Token path elements
    tokens: Vec<Token>,
}

impl HdDataSourceLocator {
    /// Creates an empty locator.
    pub fn empty() -> Self {
        Self { tokens: Vec::new() }
    }

    /// Creates a new locator from a slice of tokens.
    pub fn new(tokens: &[Token]) -> Self {
        Self {
            tokens: tokens.to_vec(),
        }
    }

    /// Creates a locator with a single token.
    pub fn from_token(token: Token) -> Self {
        Self {
            tokens: vec![token],
        }
    }

    /// Creates a locator from two tokens.
    pub fn from_tokens_2(t1: Token, t2: Token) -> Self {
        Self {
            tokens: vec![t1, t2],
        }
    }

    /// Creates a locator from three tokens.
    pub fn from_tokens_3(t1: Token, t2: Token, t3: Token) -> Self {
        Self {
            tokens: vec![t1, t2, t3],
        }
    }

    /// Returns true if the locator is empty.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Returns the number of elements (tokens).
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Returns a slice of the token elements.
    pub fn elements(&self) -> &[Token] {
        &self.tokens
    }

    /// Returns the token at the given index, or None if out of bounds.
    pub fn get_element(&self, index: usize) -> Option<&Token> {
        self.tokens.get(index)
    }

    /// Returns the first element, or None if empty.
    pub fn first_element(&self) -> Option<&Token> {
        self.tokens.first()
    }

    /// Returns the last element, or None if empty.
    pub fn last_element(&self) -> Option<&Token> {
        self.tokens.last()
    }

    /// Returns a new locator with the last element replaced.
    ///
    /// If empty, returns an identical copy.
    pub fn replace_last(&self, name: Token) -> Self {
        if self.tokens.is_empty() {
            return self.clone();
        }

        let mut tokens = self.tokens.clone();
        // Safety: We checked tokens.is_empty() above
        *tokens.last_mut().expect("tokens not empty") = name;
        Self { tokens }
    }

    /// Returns a new locator with the last element removed.
    pub fn remove_last(&self) -> Self {
        if self.tokens.is_empty() {
            return Self::empty();
        }

        let mut tokens = self.tokens.clone();
        tokens.pop();
        Self { tokens }
    }

    /// Returns a new locator with the first element removed.
    pub fn remove_first(&self) -> Self {
        if self.tokens.is_empty() {
            return Self::empty();
        }

        Self {
            tokens: self.tokens[1..].to_vec(),
        }
    }

    /// Returns a new locator with a token appended.
    pub fn append(&self, name: &Token) -> Self {
        let mut tokens = self.tokens.clone();
        tokens.push(name.clone());
        Self { tokens }
    }

    /// Returns a new locator with another locator appended.
    pub fn append_locator(&self, other: &HdDataSourceLocator) -> Self {
        let mut tokens = self.tokens.clone();
        tokens.extend_from_slice(&other.tokens);
        Self { tokens }
    }

    /// Returns a new locator with a token prepended.
    pub fn prepend(&self, name: &Token) -> Self {
        let mut tokens = vec![name.clone()];
        tokens.extend_from_slice(&self.tokens);
        Self { tokens }
    }

    /// Returns a new locator with another locator prepended.
    pub fn prepend_locator(&self, other: &HdDataSourceLocator) -> Self {
        let mut tokens = other.tokens.clone();
        tokens.extend_from_slice(&self.tokens);
        Self { tokens }
    }

    /// Returns true if this locator has the given prefix.
    ///
    /// Returns true if this locator equals the prefix.
    pub fn has_prefix(&self, prefix: &HdDataSourceLocator) -> bool {
        if prefix.tokens.len() > self.tokens.len() {
            return false;
        }

        self.tokens
            .iter()
            .zip(prefix.tokens.iter())
            .all(|(a, b)| a == b)
    }

    /// Returns the common prefix between this and another locator.
    pub fn common_prefix(&self, other: &HdDataSourceLocator) -> Self {
        let common: Vec<Token> = self
            .tokens
            .iter()
            .zip(other.tokens.iter())
            .take_while(|(a, b)| a == b)
            .map(|(a, _)| a.clone())
            .collect();

        Self { tokens: common }
    }

    /// Replaces a prefix with a new prefix.
    pub fn replace_prefix(
        &self,
        old_prefix: &HdDataSourceLocator,
        new_prefix: &HdDataSourceLocator,
    ) -> Self {
        if !self.has_prefix(old_prefix) {
            return self.clone();
        }

        let mut tokens = new_prefix.tokens.clone();
        tokens.extend_from_slice(&self.tokens[old_prefix.tokens.len()..]);
        Self { tokens }
    }

    /// Returns true if this locator intersects with another.
    ///
    /// Two locators intersect if one is a prefix of the other.
    pub fn intersects(&self, other: &HdDataSourceLocator) -> bool {
        self.has_prefix(other) || other.has_prefix(self)
    }

    /// Returns a string representation with a delimiter.
    pub fn to_string_with_delimiter(&self, delimiter: &str) -> String {
        self.tokens
            .iter()
            .map(|t| t.as_str())
            .collect::<Vec<_>>()
            .join(delimiter)
    }

    /// Computes a hash of this locator.
    pub fn compute_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

impl Default for HdDataSourceLocator {
    fn default() -> Self {
        Self::empty()
    }
}

impl Hash for HdDataSourceLocator {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tokens.hash(state);
    }
}

impl fmt::Display for HdDataSourceLocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_with_delimiter("/"))
    }
}

impl PartialOrd for HdDataSourceLocator {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HdDataSourceLocator {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.tokens.cmp(&other.tokens)
    }
}

/// A set of data source locators closed under descendancy.
///
/// If locator X is in the set, then every locator Y that has X as a prefix
/// is implicitly in the set. The set is stored as a minimal list of locators.
///
/// # Examples
///
/// ```
/// use usd_hd::data_source::*;
/// use usd_tf::Token;
///
/// let mut set = HdDataSourceLocatorSet::new();
/// set.insert(HdDataSourceLocator::new(&[Token::new("primvars")]));
///
/// // This also contains "primvars.points" implicitly
/// let points_loc = HdDataSourceLocator::new(&[
///     Token::new("primvars"),
///     Token::new("points"),
/// ]);
/// assert!(set.contains(&points_loc));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HdDataSourceLocatorSet {
    /// Sorted, minimal list of locators
    locators: Vec<HdDataSourceLocator>,
}

impl HdDataSourceLocatorSet {
    /// Creates an empty set.
    pub fn new() -> Self {
        Self {
            locators: Vec::new(),
        }
    }

    /// Creates a set from a single locator.
    pub fn from_locator(locator: HdDataSourceLocator) -> Self {
        Self {
            locators: vec![locator],
        }
    }

    /// Creates an empty set (alias for new()).
    /// Matches C++ HdDataSourceLocatorSet::UniversalSet().
    #[inline]
    pub fn empty() -> Self {
        Self::new()
    }

    /// Creates a universal set that matches everything.
    /// A universal set contains the root locator, which is an ancestor of all locators.
    pub fn universal() -> Self {
        Self::from_locator(HdDataSourceLocator::empty())
    }

    /// Returns true if this is a universal set (contains root locator).
    pub fn is_universal(&self) -> bool {
        self.locators.len() == 1 && self.locators[0].is_empty()
    }

    /// Returns true if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.locators.is_empty()
    }

    /// Inserts a locator into the set.
    ///
    /// The set is automatically normalized to maintain minimality.
    pub fn insert(&mut self, locator: HdDataSourceLocator) {
        // Remove any locators that are descendants of the new one
        self.locators.retain(|l| !l.has_prefix(&locator));

        // Don't add if it's already covered by an existing prefix
        if self.locators.iter().any(|l| locator.has_prefix(l)) {
            return;
        }

        // Insert in sorted position
        match self.locators.binary_search(&locator) {
            Ok(_) => {} // Already exists
            Err(pos) => self.locators.insert(pos, locator),
        }
    }

    /// Inserts all locators from another set.
    pub fn insert_set(&mut self, other: &HdDataSourceLocatorSet) {
        for locator in &other.locators {
            self.insert(locator.clone());
        }
    }

    /// Returns true if the set contains the locator or any ancestor.
    pub fn contains(&self, locator: &HdDataSourceLocator) -> bool {
        self.locators.iter().any(|l| locator.has_prefix(l))
    }

    /// Returns true if this set intersects with the locator.
    pub fn intersects_locator(&self, locator: &HdDataSourceLocator) -> bool {
        self.locators.iter().any(|l| l.intersects(locator))
    }

    /// Returns true if this set intersects with another set.
    pub fn intersects(&self, other: &HdDataSourceLocatorSet) -> bool {
        self.locators.iter().any(|l| other.intersects_locator(l))
    }

    /// Returns an iterator over the minimal locators in the set.
    pub fn iter(&self) -> impl Iterator<Item = &HdDataSourceLocator> {
        self.locators.iter()
    }

    /// Returns all locators from this set that intersect with the given locator.
    ///
    /// For each intersecting element in the set:
    /// - If the set element is an ancestor of the query (i.e. the query has the
    ///   set element as a prefix), the query locator itself is returned.
    /// - If the query is an ancestor of the set element, the set element is
    ///   returned.
    ///
    /// This mirrors C++ `HdDataSourceLocatorSet::Intersection`.
    pub fn intersection(&self, locator: &HdDataSourceLocator) -> Vec<HdDataSourceLocator> {
        // Find index of the first set element that intersects with `locator`.
        let start = self.locators.iter().position(|l| l.intersects(locator));
        let start = match start {
            Some(i) => i,
            None => return Vec::new(),
        };

        let mut result = Vec::new();

        // The first intersecting element may be either an ancestor or a
        // descendant of the query.  When it is an ancestor (the set element
        // is a prefix of `locator`), the C++ iterator yields `locator` itself
        // rather than the set element.  Every subsequent intersecting element
        // must be a descendant of `locator` (has `locator` as a prefix) and
        // is yielded as-is.
        let first = &self.locators[start];
        if locator.has_prefix(first) {
            // Set element is an ancestor of the query — yield the query.
            result.push(locator.clone());
        } else {
            // Set element is a descendant of the query — yield it directly.
            result.push(first.clone());
            // Walk forward collecting further descendants.
            for l in &self.locators[start + 1..] {
                if l.has_prefix(locator) {
                    result.push(l.clone());
                } else {
                    break;
                }
            }
            return result;
        }

        // After an ancestor first-element, collect all subsequent descendants.
        for l in &self.locators[start + 1..] {
            if l.has_prefix(locator) {
                result.push(l.clone());
            } else {
                break;
            }
        }

        result
    }

    /// Replaces a prefix in all locators.
    pub fn replace_prefix(
        &self,
        old_prefix: &HdDataSourceLocator,
        new_prefix: &HdDataSourceLocator,
    ) -> Self {
        let mut result = Self::new();
        for locator in &self.locators {
            result.insert(locator.replace_prefix(old_prefix, new_prefix));
        }
        result
    }
}

impl FromIterator<HdDataSourceLocator> for HdDataSourceLocatorSet {
    fn from_iter<I: IntoIterator<Item = HdDataSourceLocator>>(iter: I) -> Self {
        let mut set = Self::new();
        for locator in iter {
            set.insert(locator);
        }
        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_locator() {
        let loc = HdDataSourceLocator::empty();
        assert!(loc.is_empty());
        assert_eq!(loc.len(), 0);
    }

    #[test]
    fn test_single_token() {
        let loc = HdDataSourceLocator::from_token(Token::new("points"));
        assert_eq!(loc.len(), 1);
        assert_eq!(loc.first_element(), Some(&Token::new("points")));
    }

    #[test]
    fn test_append() {
        let loc = HdDataSourceLocator::from_token(Token::new("primvars"));
        let extended = loc.append(&Token::new("points"));
        assert_eq!(extended.len(), 2);
    }

    #[test]
    fn test_has_prefix() {
        let loc = HdDataSourceLocator::new(&[Token::new("primvars"), Token::new("points")]);
        let prefix = HdDataSourceLocator::from_token(Token::new("primvars"));
        assert!(loc.has_prefix(&prefix));
        assert!(loc.has_prefix(&loc)); // Self is prefix
    }

    #[test]
    fn test_locator_set() {
        let mut set = HdDataSourceLocatorSet::new();
        set.insert(HdDataSourceLocator::from_token(Token::new("primvars")));

        let points = HdDataSourceLocator::new(&[Token::new("primvars"), Token::new("points")]);
        assert!(set.contains(&points));
    }

    #[test]
    fn test_locator_set_minimality() {
        let mut set = HdDataSourceLocatorSet::new();

        // Insert child first
        set.insert(HdDataSourceLocator::new(&[
            Token::new("primvars"),
            Token::new("points"),
        ]));

        // Insert parent - should remove child
        set.insert(HdDataSourceLocator::from_token(Token::new("primvars")));

        assert_eq!(set.locators.len(), 1);
        assert_eq!(set.locators[0].len(), 1);
    }
}

// Allow HdRetainedTypedSampledDataSource<HdDataSourceLocator> to be used as HdSampledDataSource
impl From<HdDataSourceLocator> for usd_vt::Value {
    fn from(v: HdDataSourceLocator) -> Self {
        usd_vt::Value::from_no_hash(v)
    }
}
