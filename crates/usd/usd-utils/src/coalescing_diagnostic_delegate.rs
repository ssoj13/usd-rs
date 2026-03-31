//! Coalescing diagnostic delegate.
//!
//! Provides a diagnostic delegate that aggregates warnings and statuses
//! from Tf's diagnostic management system. Diagnostics can be coalesced
//! by invocation point for more concise output.

use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use usd_tf::CallContext;

/// The shared component in a coalesced result.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CoalescingDiagnosticDelegateSharedItem {
    /// Source line number.
    pub source_line_number: usize,
    /// Source function name.
    pub source_function: String,
    /// Source file name.
    pub source_file_name: String,
}

/// The unshared component in a coalesced result.
#[derive(Debug, Clone)]
pub struct CoalescingDiagnosticDelegateUnsharedItem {
    /// The call context where the diagnostic was issued.
    pub context: CallContext,
    /// The diagnostic commentary/message.
    pub commentary: String,
}

/// An item used in coalesced results.
#[derive(Debug, Clone)]
pub struct CoalescingDiagnosticDelegateItem {
    /// The shared component (the coalescing key).
    pub shared_item: CoalescingDiagnosticDelegateSharedItem,
    /// The set of unshared components.
    pub unshared_items: Vec<CoalescingDiagnosticDelegateUnsharedItem>,
}

/// A vector of coalesced diagnostic results.
pub type CoalescingDiagnosticDelegateVector = Vec<CoalescingDiagnosticDelegateItem>;

/// A stored diagnostic for processing.
#[derive(Debug, Clone)]
pub struct StoredDiagnostic {
    context: CallContext,
    commentary: String,
}

/// A diagnostic delegate that collects and coalesces diagnostics.
pub struct CoalescingDiagnosticDelegate {
    /// Queue of collected diagnostics.
    diagnostics: Arc<Mutex<Vec<StoredDiagnostic>>>,
}

impl CoalescingDiagnosticDelegate {
    /// Creates a new coalescing diagnostic delegate.
    pub fn new() -> Self {
        Self {
            diagnostics: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Issues an error diagnostic.
    pub fn issue_error(&self, context: CallContext, message: &str) {
        if let Ok(mut diags) = self.diagnostics.lock() {
            diags.push(StoredDiagnostic {
                context,
                commentary: message.to_string(),
            });
        }
    }

    /// Issues a status diagnostic.
    pub fn issue_status(&self, context: CallContext, message: &str) {
        if let Ok(mut diags) = self.diagnostics.lock() {
            diags.push(StoredDiagnostic {
                context,
                commentary: message.to_string(),
            });
        }
    }

    /// Issues a warning diagnostic.
    pub fn issue_warning(&self, context: CallContext, message: &str) {
        if let Ok(mut diags) = self.diagnostics.lock() {
            diags.push(StoredDiagnostic {
                context,
                commentary: message.to_string(),
            });
        }
    }

    /// Issues a fatal error diagnostic.
    pub fn issue_fatal_error(&self, _context: &CallContext, msg: &str) {
        eprintln!("FATAL ERROR: {}", msg);
    }

    /// Prints all pending diagnostics in coalesced form.
    pub fn dump_coalesced_diagnostics<W: Write>(&self, writer: &mut W) {
        let coalesced = self.take_coalesced_diagnostics();

        for item in coalesced {
            writeln!(
                writer,
                "{}:{} in {}:",
                item.shared_item.source_file_name,
                item.shared_item.source_line_number,
                item.shared_item.source_function
            )
            .ok();

            for (i, unshared) in item.unshared_items.iter().enumerate() {
                if item.unshared_items.len() > 1 {
                    writeln!(writer, "  [{}] {}", i + 1, unshared.commentary).ok();
                } else {
                    writeln!(writer, "  {}", unshared.commentary).ok();
                }
            }
        }
    }

    /// Prints all pending diagnostics without coalescing.
    pub fn dump_uncoalesced_diagnostics<W: Write>(&self, writer: &mut W) {
        let diagnostics = self.take_uncoalesced_diagnostics();

        for diag in diagnostics {
            writeln!(
                writer,
                "{}:{} in {}: {}",
                diag.context.file(),
                diag.context.line(),
                diag.context.function(),
                diag.commentary
            )
            .ok();
        }
    }

    /// Gets all pending diagnostics in coalesced form.
    pub fn take_coalesced_diagnostics(&self) -> CoalescingDiagnosticDelegateVector {
        let diagnostics = self.take_uncoalesced_diagnostics();

        let mut groups: HashMap<
            CoalescingDiagnosticDelegateSharedItem,
            Vec<CoalescingDiagnosticDelegateUnsharedItem>,
        > = HashMap::new();

        for diag in diagnostics {
            let shared = CoalescingDiagnosticDelegateSharedItem {
                source_line_number: diag.context.line() as usize,
                source_function: diag.context.function().to_string(),
                source_file_name: diag.context.file().to_string(),
            };

            let unshared = CoalescingDiagnosticDelegateUnsharedItem {
                context: diag.context,
                commentary: diag.commentary,
            };

            groups.entry(shared).or_default().push(unshared);
        }

        groups
            .into_iter()
            .map(|(shared, unshared)| CoalescingDiagnosticDelegateItem {
                shared_item: shared,
                unshared_items: unshared,
            })
            .collect()
    }

    /// Gets all pending diagnostics without coalescing.
    pub fn take_uncoalesced_diagnostics(&self) -> Vec<StoredDiagnostic> {
        if let Ok(mut diags) = self.diagnostics.lock() {
            std::mem::take(&mut *diags)
        } else {
            Vec::new()
        }
    }

    /// Returns the number of pending diagnostics.
    pub fn len(&self) -> usize {
        if let Ok(diags) = self.diagnostics.lock() {
            diags.len()
        } else {
            0
        }
    }

    /// Returns true if there are no pending diagnostics.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears all pending diagnostics without processing them.
    pub fn clear(&self) {
        if let Ok(mut diags) = self.diagnostics.lock() {
            diags.clear();
        }
    }
}

impl Default for CoalescingDiagnosticDelegate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coalescing_delegate_new() {
        let delegate = CoalescingDiagnosticDelegate::new();
        assert!(delegate.is_empty());
    }

    #[test]
    fn test_shared_item_hash() {
        use std::collections::HashSet;

        let item1 = CoalescingDiagnosticDelegateSharedItem {
            source_line_number: 42,
            source_function: "test_fn".to_string(),
            source_file_name: "test.rs".to_string(),
        };

        let item2 = CoalescingDiagnosticDelegateSharedItem {
            source_line_number: 42,
            source_function: "test_fn".to_string(),
            source_file_name: "test.rs".to_string(),
        };

        let mut set = HashSet::new();
        set.insert(item1.clone());
        assert!(set.contains(&item2));
    }
}
