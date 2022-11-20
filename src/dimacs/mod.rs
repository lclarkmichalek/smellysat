use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    num,
};

use crate::{instance::*, solver::Instance, variable_registry::VariableRegister};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DimacsError {
    #[error("malformed header")]
    MalformedHeader,
    #[error("invalid line: {0}")]
    InvalidLine(String),
    #[error("line must start with p or c: {0}:{1}")]
    UnknownLineType(usize, usize),
    #[error("io error")]
    IO(#[from] io::Error),
    #[error("not a valid value")]
    ParseError(#[from] num::ParseIntError),
}

type Result<T> = std::result::Result<T, DimacsError>;

pub fn parse(filename: &str) -> Result<Instance> {
    let file = File::open(&filename)?;
    let buffer = BufReader::new(&file);

    let mut words = buffer
        .lines()
        // Filter out lines starting with c - these are comments
        .filter(|l| match l {
            Ok(line) => line.chars().next() != Some('c'),
            // Keep errors! We need to terminate ASAP
            _ => true,
        })
        .flat_map(|line| match line {
            Ok(iter) => iter
                .split_ascii_whitespace()
                .map(|w| Ok(w.to_string()))
                .collect::<Vec<Result<String>>>(),
            Err(err) => vec![Err(err.into())],
        });

    let _header = DimacsHeader::parse(&mut words)?;

    let mut cnf: Vec<Clause> = vec![];
    let mut current_clause: Vec<Literal> = vec![];
    let mut vars = VariableRegister::new();

    for mb_word in words {
        match mb_word?.parse::<i64>()? {
            0 => {
                cnf.push(Clause::new(&current_clause));
                current_clause.clear();
            }
            encoded_value => {
                let polarity = encoded_value > 0;
                let value = if polarity {
                    encoded_value
                } else {
                    -encoded_value
                } as u64;
                let var = vars.ensure_original(&value.to_string());
                current_clause.push(Literal::new(var, polarity));
            }
        }
    }

    return Ok(Instance::new_from_clauses(cnf, vars));
}

#[derive(Debug, Clone)]
struct DimacsHeader {
    var_count: u64,
    clause_count: u64,
}

impl DimacsHeader {
    fn parse<'a, I>(words: &mut I) -> Result<Self>
    where
        I: Iterator<Item = Result<String>>,
    {
        let mut next = || match words.next() {
            Some(x) => x,
            None => Err(DimacsError::MalformedHeader),
        };

        let p = next()?;
        let cnf = next()?;
        if p != "p" || cnf != "cnf" {
            return Err(DimacsError::MalformedHeader);
        }
        let var_count = next()?;
        let clause_count = next()?;
        Ok(Self {
            var_count: var_count.parse::<u64>()?,
            clause_count: clause_count.parse::<u64>()?,
        })
    }
}
