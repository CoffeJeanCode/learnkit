//! Dependency graph for one class's micromodules — the generation pipeline
//! uses it to (a) declare what node each new block teaches, (b) check that
//! prerequisites are closed before teaching, (c) find the missing link when
//! a gate fails, and (d) enforce Δ ≤ 1 (one node per block).
//!
//! This is distinct from `domain::notebook::ConceptGraph`, which is the
//! *student-authored* graph of concepts they wrote themselves. `LessonGraph`
//! is the *generation* dependency graph defined by the syllabus / course
//! design, not by the learner.

use serde::{Deserialize, Serialize};

/// One concept / mechanism node in the class's cognitive dependency graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConceptNode {
    /// Stable identifier, used by `GeneratedSectionBlock`'s `conceptNodeId`.
    pub id: String,
    /// Human-readable label for prompts and diagnostics.
    pub label: String,
    /// Nodes that must be *closed* (passed or escalated) before this one can
    /// be taught. Empty for entry nodes.
    pub prerequisites: Vec<String>,
    /// True for the first node(s) in the class — they have no prerequisites.
    pub entry: bool,
}

/// The graph for one class. Built once when the class opens (from the
/// course syllabus / milestone), persisted as a JSON blob per document,
/// and read by `render_next_block_input` for every new block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LessonGraph {
    pub nodes: Vec<ConceptNode>,
}

impl LessonGraph {
    /// Validate structural invariants: ids unique, no cycles, every node
    /// reachable from at least one entry node, prerequisites reference
    /// real nodes only.
    pub fn validate(&self) -> Result<(), String> {
        let ids: std::collections::HashSet<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
        if ids.len() != self.nodes.len() {
            return Err("lesson_graph: duplicate node ids".into());
        }
        for n in &self.nodes {
            for p in &n.prerequisites {
                if !ids.contains(p.as_str()) {
                    return Err(format!("lesson_graph: prerequisite '{}' for '{}' not found", p, n.id));
                }
            }
        }
        // Cycle detection (simple DFS).
        let mut visited = std::collections::HashSet::new();
        let mut rec_stack = std::collections::HashSet::new();
        for n in &self.nodes {
            if !visited.contains(n.id.as_str()) {
                self.dfs_cycle(n.id.as_str(), &ids, &mut visited, &mut rec_stack)?;
            }
        }
        // Every node reachable from an entry node.
        let mut reachable = std::collections::HashSet::new();
        let mut stack: Vec<&str> = self.nodes.iter().filter(|n| n.entry).map(|n| n.id.as_str()).collect();
        while let Some(current) = stack.pop() {
            if !reachable.insert(current) { continue; }
            for n in &self.nodes {
                if n.prerequisites.contains(&current.to_string()) {
                    stack.push(n.id.as_str());
                }
            }
        }
        for n in &self.nodes {
            if !reachable.contains(n.id.as_str()) {
                return Err(format!("lesson_graph: node '{}' unreachable from any entry node", n.id));
            }
        }
        Ok(())
    }

    fn dfs_cycle(&self, current: &str, ids: &std::collections::HashSet<&str>, visited: &mut std::collections::HashSet<String>, rec: &mut std::collections::HashSet<String>) -> Result<(), String> {
        visited.insert(current.to_string());
        rec.insert(current.to_string());
        for n in &self.nodes {
            if n.id == current {
                for p in &n.prerequisites {
                    if rec.contains(p.as_str()) {
                        return Err(format!("lesson_graph: cycle involving '{}'", current));
                    }
                    if !visited.contains(p.as_str()) {
                        self.dfs_cycle(p.as_str(), ids, visited, rec)?;
                    }
                }
            }
        }
        rec.remove(&current.to_string());
        Ok(())
    }

    /// Find the node that is NOT yet closed (missing prerequisite) but whose
    /// prerequisites ARE all closed — that's the next link to teach when a
    /// gate fails (task 3.3 / 4.3 repair mode).
    pub fn missing_prerequisite(&self, failed_node_id: &str, closed_nodes: &[String]) -> Option<&ConceptNode> {
        let closed: std::collections::HashSet<&str> = closed_nodes.iter().map(|s| s.as_str()).collect();
        // Start from the failed node's prerequisites; the first one not in
        // `closed` is the missing link.
        let failed = self.nodes.iter().find(|n| n.id == failed_node_id)?;
        for p in &failed.prerequisites {
            if !closed.contains(p.as_str()) {
                return self.nodes.iter().find(|n| n.id == *p);
            }
        }
        // If all prerequisites are closed, the failure is in the node itself;
        // repair the same node at a simpler framing.
        Some(failed)
    }

    /// Whether every prerequisite of `node_id` is in `closed_nodes`.
    pub fn prerequisites_met(&self, node_id: &str, closed_nodes: &[String]) -> bool {
        let closed: std::collections::HashSet<&str> = closed_nodes.iter().map(|s| s.as_str()).collect();
        if let Some(n) = self.nodes.iter().find(|n| n.id == node_id) {
            n.prerequisites.iter().all(|p| closed.contains(p.as_str()))
        } else {
            false
        }
    }

    /// All nodes that have at least one prerequisite still open.
    pub fn open_nodes(&self, closed_nodes: &[String]) -> Vec<String> {
        let closed: std::collections::HashSet<&str> = closed_nodes.iter().map(|s| s.as_str()).collect();
        self.nodes.iter()
            .filter(|n| !n.prerequisites.is_empty() && !n.prerequisites.iter().all(|p| closed.contains(p.as_str())))
            .map(|n| n.id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> LessonGraph {
        LessonGraph {
            nodes: vec![
                ConceptNode { id: "a".into(), label: "A".into(), prerequisites: vec![], entry: true },
                ConceptNode { id: "b".into(), label: "B".into(), prerequisites: vec!["a".into()], entry: false },
                ConceptNode { id: "c".into(), label: "C".into(), prerequisites: vec!["b".into()], entry: false },
            ],
        }
    }

    #[test]
    fn validate_ok() {
        assert!(graph().validate().is_ok());
    }

    #[test]
    fn missing_prerequisite_finds_first_gap() {
        let g = graph();
        assert_eq!(g.missing_prerequisite("b", &["a".into()]).unwrap().id, "b");
        assert_eq!(g.missing_prerequisite("c", &["a".into(), "b".into()]).unwrap().id, "c");
        assert_eq!(g.missing_prerequisite("c", &["a".into()]).unwrap().id, "b");
    }

    #[test]
    fn prerequisites_met() {
        let g = graph();
        assert!(g.prerequisites_met("b", &["a".into()]));
        assert!(!g.prerequisites_met("c", &["a".into()]));
    }

    #[test]
    fn cycle_rejected() {
        let bad = LessonGraph {
            nodes: vec![
                ConceptNode { id: "a".into(), label: "A".into(), prerequisites: vec!["b".into()], entry: true },
                ConceptNode { id: "b".into(), label: "B".into(), prerequisites: vec!["a".into()], entry: false },
            ],
        };
        assert!(bad.validate().is_err());
    }
}
