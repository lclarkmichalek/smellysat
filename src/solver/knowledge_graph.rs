use std::collections::VecDeque;

use fnv::FnvHashSet;
use itertools::Itertools;
use log::trace;

use crate::instance::*;

use super::{
    backtrack::Conflict,
    clause_store::{ClauseRef, ClauseRefResolver, ClauseStore},
    trail::Trail,
};

// As we process and make deductions (through unit propogation), we would like to store the graph. This is the global knowledge graph.
pub(crate) struct KnowledgeGraph {
    vertices: Vec<Node>,
}

impl KnowledgeGraph {
    pub(crate) fn new(variable_count: usize) -> KnowledgeGraph {
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

    pub(crate) fn add_initial(&mut self, decision: Literal) {
        trace!("initial: {:?}", decision);
        self.add_decision(decision);
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
        clause: ClauseRef,
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

    pub(crate) fn inference_path(&self, conflict: &Conflict) -> Vec<Variable> {
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
        store: &ClauseStore,
        conflict: &Conflict,
    ) -> Vec<Variable> {
        let mut decisions = vec![];
        let mut seen = FnvHashSet::default();
        let mut queue = VecDeque::new();

        let conflict_var = conflict.conflicting_literal.var();
        queue.push_back(conflict_var);
        for lit in conflict.conflicting_clause.literals(&store) {
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
                for literal in clause.literals(&store) {
                    if !seen.contains(&literal.var()) {
                        queue.push_back(literal.var());
                    }
                }
            }
        }

        decisions
    }

    pub(crate) fn as_dot(&self, store: &ClauseStore, trail: &Trail) -> String {
        let mut lines = vec!["digraph knowledge_graph {".to_owned()];

        for (ix, level) in trail.search_path().iter().enumerate() {
            lines.push(format!("subgraph cluster_{} {{", ix));
            lines.push("rank = same;".to_owned());
            if let Some(decision) = level.decision {
                lines.push(format!(
                    "  {:?} [color = red, label=\"{:?}\"]",
                    decision.var(),
                    decision
                ));
            }
            for &inference in level.inferred.iter() {
                let vertex = &self.vertices[inference.var().index() as usize];
                if vertex.trigger.is_none() {
                    // if this was a unit, and inferred in decision level 0
                    lines.push(format!(
                        "  {:?} [color = black, label=\"{:?}\"]",
                        inference.var(),
                        inference
                    ));
                    continue;
                }

                lines.push(format!(
                    "  {:?} [color = grey, label=\"{:?}\"]",
                    inference.var(),
                    inference
                ));
                let trigger = vertex.trigger.unwrap();
                lines.push(format!(
                    "  {:?} -> {:?} [color = black]",
                    trigger,
                    inference.var()
                ));
                for src in store.clause_literals(vertex.clause.unwrap()) {
                    if src.var() == trigger || src.var() == inference.var() {
                        continue;
                    }
                    lines.push(format!(
                        "  {:?} -> {:?} [color = grey]",
                        src.var(),
                        inference.var()
                    ))
                }
            }
            lines.push("}".to_owned());
        }

        lines.push("}".to_owned());

        lines.join("\n")
    }

    pub(crate) fn as_dot_url(&self, store: &ClauseStore, trail: &Trail) -> String {
        vec![
            "https://edotor.net/?engine=dot#".to_owned(),
            urlencoding::encode(&self.as_dot(store, trail)).to_string(),
        ]
        .join("")
    }
}

struct Node {
    // The decision or inference that enabled unit prop to arrive here.
    // If this node was set as part of a decision, this will be None
    trigger: Option<Variable>,
    // The last decision made before unit prop arrived here
    decision: Option<Variable>,
    // The clause that allowed us to infer our way here
    clause: Option<ClauseRef>,
}
