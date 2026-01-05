use std::collections::HashMap;

use crate::instance::Variable;

#[derive(Clone, Debug)]
pub struct VariableRegister {
    variables: Vec<Variable>,
    names: HashMap<u32, String>,
    original_variables: Vec<u32>,
    literal_count: u32,
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

    pub fn get(&self, lit: Variable) -> &str {
        self.names.get(&lit.0).unwrap()
    }

    pub fn get_by_name(&self, name: &str) -> Option<Variable> {
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

    pub fn iter(&self) -> impl Iterator<Item = &Variable> + '_ {
        self.variables.iter()
    }

    pub fn iter_original(&self) -> impl Iterator<Item = Variable> + '_ {
        self.original_variables.iter().map(|ix| Variable(*ix))
    }

    pub fn count(&self) -> usize {
        self.variables.len()
    }
}
