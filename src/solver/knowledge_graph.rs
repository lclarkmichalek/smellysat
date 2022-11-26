use log::trace;

use crate::instance::*;

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
        let ix = decision.var().index() as usize;
        let mut v = self.vertices.get_mut(ix).unwrap();
        v.trigger = Some(decision.var());
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
        let ix = inferred.var().index() as usize;
        let mut v = self.vertices.get_mut(ix).unwrap();
        v.trigger = Some(trigger.var());
        v.decision = decision.map(|l| l.var());
        v.clause = Some(clause);
    }
}

struct Node<'c> {
    // The decision or inference that enabled unit prop to arrive here
    trigger: Option<Variable>,
    // The last decision made before unit prop arrived here
    decision: Option<Variable>,
    // The clause that allowed us to infer our way here
    clause: Option<&'c Clause>,
}
