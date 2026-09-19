use super::{mm, Rule, RuleInput};
use crate::diagnostics::{Diagnostic, Severity};

/// FAB-101: two parts share volume. Touching is fine; overlapping is not,
/// unless the parts declared each other exempt (a back inside its groove).
pub struct PartCollision;

impl Rule for PartCollision {
    fn id(&self) -> &'static str {
        "FAB-101"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        let parts = input.parts;
        for i in 0..parts.len() {
            for j in (i + 1)..parts.len() {
                let (a, b) = (&parts[i], &parts[j]);
                if a.overlap_exempt.contains(&b.id) || b.overlap_exempt.contains(&a.id) {
                    continue;
                }
                if let Some(x) = a.aabb.intersection(&b.aabb) {
                    let s = x.size();
                    out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Error,
                            format!(
                                "{} ({}) y {} ({}) se superponen {}×{}×{} mm",
                                a.id,
                                a.name,
                                b.id,
                                b.name,
                                mm(s.0),
                                mm(s.1),
                                mm(s.2)
                            ),
                        )
                        .entity(a.id.clone())
                        .location(b.id.clone())
                        .suggestion("Revisá las dimensiones o la posición de los componentes que las generan."),
                    );
                }
            }
        }
        out
    }
}
