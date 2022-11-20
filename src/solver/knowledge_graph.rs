use fnv::{FnvHashMap, FnvHashSet};

use crate::instance::*;

// As we process and make deductions (through unit propogation), we would like to store the graph. This is the global knowledge graph.
pub(crate) struct KnowledgeGraph {
    vertices: FnvHashSet<Literal>,
    decisions: FnvHashSet<Literal>,
    // From A to B, where B is inferred
    edge_to: FnvHashMap<Literal, Literal>,
    // From B to A, where B is inferred
    edge_from: FnvHashMap<Literal, Literal>,
}

impl KnowledgeGraph {
    pub(crate) fn new() -> KnowledgeGraph {
        KnowledgeGraph {
            vertices: FnvHashSet::default(),
            decisions: FnvHashSet::default(),
            edge_to: FnvHashMap::default(),
            edge_from: FnvHashMap::default(),
        }
    }

    pub(crate) fn add_decision(&mut self, decision: Literal) {
        self.vertices.insert(decision);
        self.decisions.insert(decision);
    }

    pub(crate) fn add_inferred(&mut self, inferred: Literal, clause: &Clause) {
        self.vertices.insert(inferred);
        for &literal in clause.literals() {
            if literal == inferred {
                continue;
            }

            self.edge_to.insert(literal, inferred);
            self.edge_from.insert(literal, inferred);
        }
    }
}

struct ImplicationTrail {
    trigger_decision: Literal,
    relevant_decisions: Vec<Literal>,
    trail: Vec<Literal>,
}

impl ImplicationTrail {}
