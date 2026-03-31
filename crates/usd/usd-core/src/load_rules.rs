//! Stage load rules for controlling payload loading.
//!
//! StageLoadRules determine which payloads are loaded or unloaded on a stage.
//! They allow fine-grained control over what parts of a scene are loaded.

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use usd_sdf::Path;

// ============================================================================
// Rule
// ============================================================================

/// Load rule for a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Rule {
    /// Load all payloads at this path and descendants.
    #[default]
    AllRule,
    /// Load only payloads at this exact path.
    OnlyRule,
    /// Don't load any payloads at this path or descendants.
    NoneRule,
}

// ============================================================================
// StageLoadRules
// ============================================================================

/// Rules controlling payload loading on a UsdStage.
///
/// StageLoadRules is a mapping from paths to load rules. When determining
/// whether a prim's payload should be loaded, the stage consults these rules.
/// The rule that applies to a prim is the rule for the nearest ancestor path
/// in the rules (or the pseudo-root if no ancestors are in the rules).
#[derive(Debug, Clone, Default)]
pub struct StageLoadRules {
    /// Path -> Rule mapping
    rules: HashMap<Path, Rule>,
}

impl StageLoadRules {
    fn longest_prefix_rule(&self, path: &Path) -> Option<(Path, Rule)> {
        self.rules
            .iter()
            .filter(|(rule_path, _)| path.has_prefix(rule_path))
            .max_by_key(|(rule_path, _)| rule_path.get_path_element_count())
            .map(|(rule_path, rule)| (rule_path.clone(), *rule))
    }

    fn has_non_none_descendant_rule(&self, path: &Path) -> bool {
        self.rules
            .iter()
            .any(|(rule_path, rule)| *rule != Rule::NoneRule && *rule_path != *path && rule_path.has_prefix(path))
    }

    /// Creates default load rules (load everything).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates rules that load all payloads.
    pub fn load_all() -> Self {
        Self::default()
    }

    /// Creates rules that load no payloads.
    pub fn load_none() -> Self {
        let mut rules = Self::new();
        rules.set_rule(Path::absolute_root(), Rule::NoneRule);
        rules
    }

    /// Adds a rule for the given path.
    pub fn add_rule(&mut self, path: Path, rule: Rule) {
        self.rules.insert(path, rule);
    }

    /// Sets a rule for the given path (alias for add_rule).
    pub fn set_rule(&mut self, path: Path, rule: Rule) {
        self.add_rule(path, rule);
    }

    /// Removes the rule for the given path.
    pub fn remove_rule(&mut self, path: &Path) {
        self.rules.remove(path);
    }

    /// Returns the rule for the given path, or None if not set.
    pub fn get_rule(&self, path: &Path) -> Option<Rule> {
        self.rules.get(path).copied()
    }

    /// Returns the effective rule for a path following OpenUSD prefix/direct-child
    /// rule resolution semantics.
    pub fn get_effective_rule(&self, path: &Path) -> Rule {
        if self.rules.is_empty() {
            return Rule::AllRule;
        }

        let Some((prefix_path, prefix_rule)) = self.longest_prefix_rule(path) else {
            return Rule::AllRule;
        };

        if prefix_rule == Rule::AllRule {
            return Rule::AllRule;
        }

        if prefix_path == *path && prefix_rule == Rule::OnlyRule {
            return Rule::OnlyRule;
        }

        let mut minimal_descendants: Vec<(&Path, Rule)> = Vec::new();
        for (rule_path, rule) in &self.rules {
            if *rule_path == *path || !rule_path.has_prefix(path) {
                continue;
            }

            let shadowed = self.rules.keys().any(|other_path| {
                other_path != rule_path
                    && *other_path != *path
                    && other_path.has_prefix(path)
                    && rule_path.has_prefix(other_path)
            });

            if !shadowed {
                minimal_descendants.push((rule_path, *rule));
            }
        }

        if minimal_descendants.is_empty() {
            return Rule::NoneRule;
        }

        for (_rule_path, rule) in minimal_descendants {
            if rule == Rule::OnlyRule || rule == Rule::AllRule {
                return Rule::OnlyRule;
            }
        }

        Rule::NoneRule
    }

    /// Returns true if the given path's payload should be loaded.
    ///
    /// Matches C++ `UsdStageLoadRules::IsLoaded()`.
    /// For `OnlyRule`: loads only the exact path where the rule is set,
    /// not its descendants. If an ancestor has OnlyRule, descendants
    /// are NOT loaded (the rule only applies to the annotated path).
    pub fn is_loaded(&self, path: &Path) -> bool {
        self.get_effective_rule(path) != Rule::NoneRule
    }

    /// Returns true if all payloads should be loaded (no rules set).
    pub fn is_load_all(&self) -> bool {
        self.rules.is_empty()
    }

    /// Returns the number of rules.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Returns true if there are no rules.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Clears all rules.
    pub fn clear(&mut self) {
        self.rules.clear();
    }

    /// Returns iterator over all rules.
    pub fn iter(&self) -> impl Iterator<Item = (&Path, &Rule)> {
        self.rules.iter()
    }

    /// Returns a set of all loaded paths (matches C++ GetLoadSet).
    pub fn get_loaded_paths(&self) -> std::collections::HashSet<Path> {
        let mut loaded = std::collections::HashSet::new();
        for (path, rule) in self.rules.iter() {
            match rule {
                Rule::AllRule | Rule::OnlyRule => {
                    loaded.insert(path.clone());
                }
                Rule::NoneRule => {}
            }
        }
        loaded
    }

    /// Minimizes the rule set by removing redundant rules.
    pub fn minimize(&mut self) {
        // Remove rules that have no effect (same as parent's effective rule)
        let paths: Vec<Path> = self.rules.keys().cloned().collect();
        for path in paths {
            if let Some(rule) = self.rules.get(&path).copied() {
                // Get parent's effective rule
                let parent = path.get_parent_path();
                let parent_rule = if parent.is_empty() {
                    Rule::AllRule
                } else {
                    self.get_effective_rule(&parent)
                };
                // If same as parent, remove
                if rule == parent_rule {
                    self.rules.remove(&path);
                }
            }
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rule::AllRule => write!(f, "AllRule"),
            Rule::OnlyRule => write!(f, "OnlyRule"),
            Rule::NoneRule => write!(f, "NoneRule"),
        }
    }
}

impl StageLoadRules {
    /// Load the given path and all its descendants.
    ///
    /// Removes any descendant rules that would contradict this.
    /// Matches C++ `UsdStageLoadRules::LoadWithDescendants()`.
    pub fn load_with_descendants(&mut self, path: &Path) {
        // Remove descendant rules that are overridden
        let paths_to_remove: Vec<Path> = self
            .rules
            .keys()
            .filter(|p| p.has_prefix(path) && *p != path)
            .cloned()
            .collect();
        for p in paths_to_remove {
            self.rules.remove(&p);
        }
        // Ensure ancestors are loaded (stop at root)
        let mut ancestor = path.get_parent_path();
        while !ancestor.is_empty() && !ancestor.is_absolute_root_path() {
            self.rules.entry(ancestor.clone()).or_insert(Rule::OnlyRule);
            ancestor = ancestor.get_parent_path();
        }
        self.add_rule(path.clone(), Rule::AllRule);
    }

    /// Load the given path but none of its descendants.
    ///
    /// Matches C++ `UsdStageLoadRules::LoadWithoutDescendants()`.
    pub fn load_without_descendants(&mut self, path: &Path) {
        // Remove descendant rules that are overridden
        let paths_to_remove: Vec<Path> = self
            .rules
            .keys()
            .filter(|p| p.has_prefix(path) && *p != path)
            .cloned()
            .collect();
        for p in paths_to_remove {
            self.rules.remove(&p);
        }
        // Ensure ancestors are loaded (stop at root)
        let mut ancestor = path.get_parent_path();
        while !ancestor.is_empty() && !ancestor.is_absolute_root_path() {
            self.rules.entry(ancestor.clone()).or_insert(Rule::OnlyRule);
            ancestor = ancestor.get_parent_path();
        }
        self.add_rule(path.clone(), Rule::OnlyRule);
    }

    /// Unload the given path and all its descendants.
    ///
    /// Matches C++ `UsdStageLoadRules::Unload()`.
    pub fn unload(&mut self, path: &Path) {
        // Remove descendant rules that are overridden
        let paths_to_remove: Vec<Path> = self
            .rules
            .keys()
            .filter(|p| p.has_prefix(path) && *p != path)
            .cloned()
            .collect();
        for p in paths_to_remove {
            self.rules.remove(&p);
        }
        self.add_rule(path.clone(), Rule::NoneRule);
    }

    /// Returns true if the given path and all descendants are loaded.
    ///
    /// Matches C++ `UsdStageLoadRules::IsLoadedWithAllDescendants()`.
    pub fn is_loaded_with_all_descendants(&self, path: &Path) -> bool {
        if self.rules.is_empty() {
            return true;
        }

        if let Some((_prefix_path, prefix_rule)) = self.longest_prefix_rule(path) {
            if prefix_rule != Rule::AllRule {
                return false;
            }
        }

        !self
            .rules
            .iter()
            .any(|(rule_path, rule)| rule_path.has_prefix(path) && *rule != Rule::AllRule)
    }

    /// Returns true if the given path is loaded but none of its descendants.
    ///
    /// Matches C++ `UsdStageLoadRules::IsLoadedWithNoDescendants()`.
    pub fn is_loaded_with_no_descendants(&self, path: &Path) -> bool {
        if self.rules.is_empty() {
            return false;
        }

        if self.rules.get(path) != Some(&Rule::OnlyRule) {
            return false;
        }

        !self.has_non_none_descendant_rule(path)
    }

    /// Returns all rules as sorted pairs.
    ///
    /// Matches C++ `UsdStageLoadRules::GetRules()`.
    pub fn get_rules(&self) -> Vec<(Path, Rule)> {
        let mut rules: Vec<(Path, Rule)> =
            self.rules.iter().map(|(p, r)| (p.clone(), *r)).collect();
        rules.sort_by(|a, b| a.0.cmp(&b.0));
        rules
    }

    /// Sets rules from sorted pairs.
    ///
    /// Matches C++ `UsdStageLoadRules::SetRules()`.
    pub fn set_rules(&mut self, rules: Vec<(Path, Rule)>) {
        self.rules.clear();
        for (path, rule) in rules {
            self.rules.insert(path, rule);
        }
    }

    /// Swap contents with another StageLoadRules.
    pub fn swap(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.rules, &mut other.rules);
    }
}

impl fmt::Display for StageLoadRules {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StageLoadRules: [")?;
        let rules = self.get_rules();
        for (i, (path, rule)) in rules.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "({}, {})", path.get_string(), rule)?;
        }
        write!(f, "]")
    }
}

impl Hash for StageLoadRules {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash sorted rules for deterministic hashing
        let rules = self.get_rules();
        rules.len().hash(state);
        for (path, rule) in &rules {
            path.hash(state);
            std::mem::discriminant(rule).hash(state);
        }
    }
}

impl PartialEq for StageLoadRules {
    fn eq(&self, other: &Self) -> bool {
        self.rules == other.rules
    }
}

impl Eq for StageLoadRules {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_all() {
        let rules = StageLoadRules::load_all();
        assert!(rules.is_load_all());
        assert!(rules.is_empty());
    }

    #[test]
    fn test_load_none() {
        let rules = StageLoadRules::load_none();
        assert!(!rules.is_load_all());

        let path = Path::from_string("/World").unwrap();
        assert_eq!(rules.get_effective_rule(&path), Rule::NoneRule);
    }

    #[test]
    fn test_add_rule() {
        let mut rules = StageLoadRules::new();
        let path = Path::from_string("/World").unwrap();

        rules.add_rule(path.clone(), Rule::NoneRule);
        assert_eq!(rules.get_rule(&path), Some(Rule::NoneRule));
    }

    #[test]
    fn test_effective_rule_inheritance() {
        let mut rules = StageLoadRules::new();
        let world = Path::from_string("/World").unwrap();
        let child = Path::from_string("/World/Child").unwrap();

        rules.add_rule(world, Rule::NoneRule);
        assert_eq!(rules.get_effective_rule(&child), Rule::NoneRule);
    }

    #[test]
    fn test_is_loaded() {
        let mut rules = StageLoadRules::new();
        let world = Path::from_string("/World").unwrap();
        let child = Path::from_string("/World/Child").unwrap();

        // Default: everything loaded
        assert!(rules.is_loaded(&world));
        assert!(rules.is_loaded(&child));

        // Set none rule
        rules.add_rule(world.clone(), Rule::NoneRule);
        assert!(!rules.is_loaded(&world));
        assert!(!rules.is_loaded(&child));
    }

    #[test]
    fn test_load_with_descendants() {
        let mut rules = StageLoadRules::load_none();
        let kitchen = Path::from_string("/World/sets/kitchen").unwrap();

        rules.load_with_descendants(&kitchen);
        assert!(rules.is_loaded(&kitchen));
        assert!(rules.is_loaded_with_all_descendants(&kitchen));
        // Ancestor should have OnlyRule ensuring it is at least loaded
        let sets = Path::from_string("/World/sets").unwrap();
        assert_eq!(rules.get_rule(&sets), Some(Rule::OnlyRule));
    }

    #[test]
    fn test_load_without_descendants() {
        let mut rules = StageLoadRules::load_none();
        let kitchen = Path::from_string("/World/sets/kitchen").unwrap();

        rules.load_without_descendants(&kitchen);
        assert!(rules.is_loaded_with_no_descendants(&kitchen));
        assert!(!rules.is_loaded_with_all_descendants(&kitchen));
    }

    #[test]
    fn test_unload() {
        let mut rules = StageLoadRules::new();
        let world = Path::from_string("/World").unwrap();
        let child = Path::from_string("/World/Child").unwrap();

        // Add a child rule, then unload world (should remove child rule)
        rules.add_rule(child.clone(), Rule::AllRule);
        rules.unload(&world);
        assert!(!rules.is_loaded(&world));
        assert!(rules.get_rule(&child).is_none()); // child rule removed
    }

    #[test]
    fn test_get_rules_sorted() {
        let mut rules = StageLoadRules::new();
        rules.add_rule(Path::from_string("/Z").unwrap(), Rule::NoneRule);
        rules.add_rule(Path::from_string("/A").unwrap(), Rule::AllRule);
        rules.add_rule(Path::from_string("/M").unwrap(), Rule::OnlyRule);

        let sorted = rules.get_rules();
        assert_eq!(sorted[0].0.get_string(), "/A");
        assert_eq!(sorted[1].0.get_string(), "/M");
        assert_eq!(sorted[2].0.get_string(), "/Z");
    }

    #[test]
    fn test_set_rules() {
        let mut rules = StageLoadRules::new();
        rules.set_rules(vec![
            (Path::from_string("/A").unwrap(), Rule::AllRule),
            (Path::from_string("/B").unwrap(), Rule::NoneRule),
        ]);
        assert_eq!(rules.len(), 2);
        assert_eq!(
            rules.get_rule(&Path::from_string("/A").unwrap()),
            Some(Rule::AllRule)
        );
    }

    #[test]
    fn test_swap() {
        let mut rules_a = StageLoadRules::new();
        rules_a.add_rule(Path::from_string("/A").unwrap(), Rule::AllRule);
        let mut rules_b = StageLoadRules::new();
        rules_b.add_rule(Path::from_string("/B").unwrap(), Rule::NoneRule);

        rules_a.swap(&mut rules_b);
        assert!(
            rules_a
                .get_rule(&Path::from_string("/B").unwrap())
                .is_some()
        );
        assert!(
            rules_b
                .get_rule(&Path::from_string("/A").unwrap())
                .is_some()
        );
    }

    #[test]
    fn test_display_rule() {
        assert_eq!(Rule::AllRule.to_string(), "AllRule");
        assert_eq!(Rule::OnlyRule.to_string(), "OnlyRule");
        assert_eq!(Rule::NoneRule.to_string(), "NoneRule");
    }

    #[test]
    fn test_display_stage_load_rules() {
        let mut rules = StageLoadRules::new();
        rules.add_rule(Path::from_string("/World").unwrap(), Rule::AllRule);
        let s = rules.to_string();
        assert!(s.contains("StageLoadRules:"));
        assert!(s.contains("/World"));
        assert!(s.contains("AllRule"));
    }
}
