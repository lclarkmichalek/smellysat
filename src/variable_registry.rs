use std::collections::HashMap;

use crate::instance::Variable;

#[derive(Clone, Debug)]
pub(crate) struct VariableRegister {
    variables: Vec<Variable>,
    names: HashMap<u64, String>,
    original_variables: Vec<u64>,
    literal_count: u64,
}

impl VariableRegister {
    pub(crate) fn new() -> VariableRegister {
        VariableRegister {
            variables: vec![],
            names: HashMap::new(),
            original_variables: vec![],
            literal_count: 0,
        }
    }

    pub(crate) fn get(&self, lit: Variable) -> &str {
        self.names.get(&lit.0).unwrap()
    }

    pub(crate) fn get_by_name(&self, name: &str) -> Option<Variable> {
        for (&ix, n) in &self.names {
            if n == name {
                return Some(Variable(ix));
            }
        }
        None
    }

    pub(crate) fn create_original(&mut self, name: &str) -> Variable {
        let ix = self.literal_count;
        self.variables.push(Variable(ix));
        self.names.insert(ix, name.to_string());
        self.original_variables.push(ix);
        self.literal_count += 1;
        Variable(ix)
    }

    pub(crate) fn ensure_original(&mut self, name: &str) -> Variable {
        match self.get_by_name(name) {
            Some(var) => var,
            None => self.create_original(name),
        }
    }

    pub(crate) fn create_tseitin(&mut self) -> Variable {
        let ix = self.literal_count;
        self.variables.push(Variable(ix));
        self.names.insert(ix, format!("t#{}", ix));
        self.literal_count += 1;
        Variable(ix)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Variable> + '_ {
        self.variables.iter()
    }

    pub(crate) fn iter_original(&self) -> impl Iterator<Item = Variable> + '_ {
        self.original_variables.iter().map(|ix| Variable(*ix))
    }
}
