//! Sprint commitment, completion, spillover, and velocity.

use std::collections::HashSet;

/// Aggregated sprint delivery metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct SprintMetrics {
    pub sprint_id: String,
    pub committed: u32,
    pub completed: u32,
    pub spillover: u32,
    pub scope_added: u32,
    pub scope_removed: u32,
    pub velocity_points: Option<f64>,
}

/// Compute sprint metrics from commitment / completion / mid-sprint scope sets.
///
/// **Spillover** is the count of issues that were committed at sprint start and are
/// not Done at sprint end (including issues removed mid-sprint without completion).
pub fn compute_sprint_metrics(
    sprint_id: &str,
    committed: &[&str],
    completed: &[&str],
    added_mid: &[&str],
    removed_mid: &[&str],
    velocity_points: Option<f64>,
) -> SprintMetrics {
    let completed_set: HashSet<&str> = completed.iter().copied().collect();
    let spillover = committed
        .iter()
        .filter(|key| !completed_set.contains(*key))
        .count() as u32;

    SprintMetrics {
        sprint_id: sprint_id.to_string(),
        committed: committed.len() as u32,
        completed: completed.len() as u32,
        spillover,
        scope_added: added_mid.len() as u32,
        scope_removed: removed_mid.len() as u32,
        velocity_points,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprint_metrics_counts_commitment_completion_and_spillover() {
        let m = compute_sprint_metrics(
            "s1",
            /* committed keys */ &["A-1", "A-2", "A-3"],
            /* completed in sprint */ &["A-1", "A-2"],
            /* added mid */ &["A-4"],
            /* removed mid */ &["A-3"],
            /* points completed */ Some(5.0),
        );
        assert_eq!(m.committed, 3);
        assert_eq!(m.completed, 2);
        assert_eq!(m.spillover, 1); // A-3 removed or unfinished per definition in docstring
        assert_eq!(m.scope_added, 1);
        assert_eq!(m.scope_removed, 1);
        assert_eq!(m.velocity_points, Some(5.0));
        assert_eq!(m.sprint_id, "s1");
    }
}
