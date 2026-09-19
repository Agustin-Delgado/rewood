//! Structured findings from constraints, rules and validation.
//!
//! Every diagnostic carries a stable code so the UI can link it to help and
//! tests can assert on it, plus a human-readable message (in Spanish, the
//! product language) and, when possible, a suggestion.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Info,
    Warning,
    /// Manufacturing may still be generated; the operator decides.
    Error,
    /// Manufacturing is blocked.
    Fatal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    /// Entity the finding is about: a part id, a joint id, a parameter name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    /// Finer location inside the entity: an operation id, a face.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// A concrete change to the spec that resolves the finding, when the
    /// engine can name one. Applied with [`crate::spec::apply_fix`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

/// One field of one component (or of the furniture root) set to a value.
/// Small on purpose: a fix is a suggestion the operator applies with one
/// click and then reads the new findings, not a solver.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fix {
    /// Button text, Spanish: "Dividir en 2 bahías".
    pub label: String,
    /// Component id the field belongs to; empty = the furniture root.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub component: String,
    /// Dotted path inside the component: `bays`, `origin.x`,
    /// `handle.fromEdge`, `slide.hardware`.
    pub field: String,
    /// New value; `null` removes the field.
    pub value: serde_json::Value,
}

impl Diagnostic {
    pub fn new(code: &str, severity: Severity, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            code: code.to_string(),
            severity,
            entity: None,
            location: None,
            message: message.into(),
            suggestion: None,
            fix: None,
        }
    }

    pub fn entity(mut self, e: impl Into<String>) -> Diagnostic {
        self.entity = Some(e.into());
        self
    }

    pub fn location(mut self, l: impl Into<String>) -> Diagnostic {
        self.location = Some(l.into());
        self
    }

    pub fn suggestion(mut self, s: impl Into<String>) -> Diagnostic {
        self.suggestion = Some(s.into());
        self
    }

    /// A fix the caller may or may not have found.
    pub fn fix_opt(self, fix: Option<(String, String, String, serde_json::Value)>) -> Diagnostic {
        match fix {
            Some((label, component, field, value)) => self.fix(label, component, field, value),
            None => self,
        }
    }

    pub fn fix(
        mut self,
        label: impl Into<String>,
        component: impl Into<String>,
        field: impl Into<String>,
        value: serde_json::Value,
    ) -> Diagnostic {
        self.fix = Some(Fix {
            label: label.into(),
            component: component.into(),
            field: field.into(),
            value,
        });
        self
    }
}

/// Ordered collection with the aggregate the pipeline cares about.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Diagnostics {
    pub items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn push(&mut self, d: Diagnostic) {
        self.items.push(d);
    }

    pub fn extend(&mut self, other: Diagnostics) {
        self.items.extend(other.items);
    }

    pub fn has_fatal(&self) -> bool {
        self.items.iter().any(|d| d.severity == Severity::Fatal)
    }

    pub fn max_severity(&self) -> Option<Severity> {
        self.items.iter().map(|d| d.severity).max()
    }

    pub fn count(&self, s: Severity) -> usize {
        self.items.iter().filter(|d| d.severity == s).count()
    }

    /// Stable order: severity descending, then code, then entity/location.
    /// Rules run in a fixed order already, but sorting makes the output
    /// independent of that order should it ever change.
    pub fn sort(&mut self) {
        self.items.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.code.cmp(&b.code))
                .then_with(|| a.entity.cmp(&b.entity))
                .then_with(|| a.location.cmp(&b.location))
                .then_with(|| a.message.cmp(&b.message))
        });
    }
}
