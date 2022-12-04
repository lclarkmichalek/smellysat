use std::collections::VecDeque;

use fnv::FnvHashSet;
use log::trace;

use crate::instance::*;

use super::backtrack::Conflict;

// As we process and make deductions (through unit propogation), we would like to store the graph. This is the global knowledge graph.
pub(crate) struct KnowledgeGraph<'c> {
    vertices: Vec<Node<'c>>,
}

impl<'c> KnowledgeGraph<'c> {
    pub(crate) fn new(variable_count: usize) -> KnowledgeGraph<'c> {
        KnowledgeGraph {
            vertices: (0..variable_count)
                .map(|_| Node {
                    trigger: None,
                    decision: None,
                    clause: None,
                })
                .collect(),
        }
    }

    pub(crate) fn add_decision(&mut self, decision: Literal) {
        trace!("decision: {:?}", decision);
        let v = &mut self.vertices[decision.var().index() as usize];
        v.trigger = None;
        v.decision = Some(decision.var());
        v.clause = None;
    }

    pub(crate) fn add_inferred(
        &mut self,
        inferred: Literal,
        trigger: Literal,
        decision: Option<Literal>,
        clause: &'c Clause,
    ) {
        trace!("inference: {:?}", inferred);
        let v = &mut self.vertices[inferred.var().index() as usize];
        v.trigger = Some(trigger.var());
        v.decision = decision.map(|l| l.var());
        v.clause = Some(clause);
    }

    pub(crate) fn remove(&mut self, literals: &Vec<Literal>) {
        for literal in literals.iter() {
            let v = &mut self.vertices[literal.var().index() as usize];
            v.trigger = None;
            v.decision = None;
            v.clause = None;
        }
    }

    pub(crate) fn inference_path(&self, conflict: &Conflict<'c>) -> Vec<Variable> {
        let mut path = vec![conflict.conflicting_literal.var()];
        let mut ptr = &self.vertices[conflict.conflicting_literal.var().index() as usize];
        loop {
            match ptr.trigger {
                None => break,
                Some(var) => {
                    path.push(var);
                    ptr = &self.vertices[var.index() as usize];
                }
            }
        }
        path
    }

    pub(crate) fn find_implicated_decision_variables(
        &self,
        conflict: &Conflict<'c>,
    ) -> Vec<Variable> {
        let mut decisions = vec![];
        let mut seen = FnvHashSet::default();
        let mut queue = VecDeque::new();

        let conflict_var = conflict.conflicting_literal.var();
        queue.push_back(conflict_var);
        for lit in conflict.conflicting_clause.literals() {
            queue.push_back(lit.var());
        }

        loop {
            let var = match queue.pop_front() {
                None => break,
                Some(x) => x,
            };
            if seen.contains(&var) {
                continue;
            }
            seen.insert(var);
            let node = &self.vertices[var.index() as usize];

            if node.trigger.is_none() {
                decisions.push(var);
                continue;
            }

            if let Some(clause) = node.clause {
                for literal in clause.literals() {
                    if !seen.contains(&literal.var()) {
                        queue.push_back(literal.var());
                    }
                }
            }
        }

        decisions
    }
}

struct Node<'c> {
    // The decision or inference that enabled unit prop to arrive here.
    // If this node was set as part of a decision, this will be None
    trigger: Option<Variable>,
    // The last decision made before unit prop arrived here
    decision: Option<Variable>,
    // The clause that allowed us to infer our way here
    clause: Option<&'c Clause>,
}
