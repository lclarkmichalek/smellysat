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
    _var_count: u64,
    _clause_count: u64,
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
            _var_count: var_count.parse::<u64>()?,
            _clause_count: clause_count.parse::<u64>()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp_cnf(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        file.write_all(content.as_bytes())
            .expect("Failed to write temp file");
        file.flush().expect("Failed to flush temp file");
        file
    }

    #[test]
    fn test_parse_simple_cnf() {
        let content = "p cnf 3 2\n1 2 0\n-1 3 0\n";
        let file = write_temp_cnf(content);

        let instance = parse(file.path().to_str().unwrap()).expect("Failed to parse");

        assert_eq!(instance.clauses.len(), 2);
        assert_eq!(instance.clauses[0].len(), 2);
        assert_eq!(instance.clauses[1].len(), 2);
    }

    #[test]
    fn test_parse_with_comments() {
        let content = "c This is a comment\nc Another comment\np cnf 2 1\n1 -2 0\n";
        let file = write_temp_cnf(content);

        let instance = parse(file.path().to_str().unwrap()).expect("Failed to parse");

        assert_eq!(instance.clauses.len(), 1);
        assert_eq!(instance.clauses[0].len(), 2);
    }

    #[test]
    fn test_parse_multiline_comments() {
        let content = "c FILE: test.cnf\nc\nc SOURCE: Test\nc\np cnf 1 1\n1 0\n";
        let file = write_temp_cnf(content);

        let instance = parse(file.path().to_str().unwrap()).expect("Failed to parse");

        assert_eq!(instance.clauses.len(), 1);
    }

    #[test]
    fn test_parse_negative_literals() {
        let content = "p cnf 3 1\n-1 -2 -3 0\n";
        let file = write_temp_cnf(content);

        let instance = parse(file.path().to_str().unwrap()).expect("Failed to parse");

        assert_eq!(instance.clauses.len(), 1);
        assert_eq!(instance.clauses[0].len(), 3);

        // All literals should be negative (polarity = false)
        for lit in instance.clauses[0].literals() {
            assert!(!lit.polarity(), "Expected negative literal");
        }
    }

    #[test]
    fn test_parse_mixed_polarity() {
        let content = "p cnf 4 1\n1 -2 3 -4 0\n";
        let file = write_temp_cnf(content);

        let instance = parse(file.path().to_str().unwrap()).expect("Failed to parse");

        let lits = instance.clauses[0].literals();
        assert_eq!(lits.len(), 4);
    }

    #[test]
    fn test_parse_unit_clauses() {
        let content = "p cnf 3 3\n1 0\n-2 0\n3 0\n";
        let file = write_temp_cnf(content);

        let instance = parse(file.path().to_str().unwrap()).expect("Failed to parse");

        assert_eq!(instance.clauses.len(), 3);
        for clause in &instance.clauses {
            assert!(clause.is_unit(), "Expected unit clause");
        }
    }

    #[test]
    fn test_parse_empty_file_fails() {
        let content = "";
        let file = write_temp_cnf(content);

        let result = parse(file.path().to_str().unwrap());

        assert!(result.is_err());
        match result {
            Err(DimacsError::MalformedHeader) => (),
            _ => panic!("Expected MalformedHeader error"),
        }
    }

    #[test]
    fn test_parse_missing_header_fails() {
        let content = "1 2 0\n";
        let file = write_temp_cnf(content);

        let result = parse(file.path().to_str().unwrap());

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_header_format_fails() {
        let content = "p sat 3 2\n1 2 0\n";
        let file = write_temp_cnf(content);

        let result = parse(file.path().to_str().unwrap());

        assert!(result.is_err());
        match result {
            Err(DimacsError::MalformedHeader) => (),
            _ => panic!("Expected MalformedHeader error"),
        }
    }

    #[test]
    fn test_parse_incomplete_header_fails() {
        let content = "p cnf 3\n";
        let file = write_temp_cnf(content);

        let result = parse(file.path().to_str().unwrap());

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_nonexistent_file_fails() {
        let result = parse("/nonexistent/path/to/file.cnf");

        assert!(result.is_err());
        match result {
            Err(DimacsError::IO(_)) => (),
            _ => panic!("Expected IO error"),
        }
    }

    #[test]
    fn test_parse_whitespace_variations() {
        // Multiple spaces, tabs, and varying whitespace
        let content = "p   cnf   3   2\n1  2   3 0\n-1\t-2\t0\n";
        let file = write_temp_cnf(content);

        let instance = parse(file.path().to_str().unwrap()).expect("Failed to parse");

        assert_eq!(instance.clauses.len(), 2);
        assert_eq!(instance.clauses[0].len(), 3);
        assert_eq!(instance.clauses[1].len(), 2);
    }

    #[test]
    fn test_parse_clause_spanning_lines() {
        // Clause literals can span multiple lines before terminating 0
        let content = "p cnf 4 1\n1 2\n3 4\n0\n";
        let file = write_temp_cnf(content);

        let instance = parse(file.path().to_str().unwrap()).expect("Failed to parse");

        assert_eq!(instance.clauses.len(), 1);
        assert_eq!(instance.clauses[0].len(), 4);
    }

    #[test]
    fn test_parse_multiple_clauses_same_line() {
        // Multiple clauses on the same line
        let content = "p cnf 2 3\n1 0 2 0 -1 -2 0\n";
        let file = write_temp_cnf(content);

        let instance = parse(file.path().to_str().unwrap()).expect("Failed to parse");

        assert_eq!(instance.clauses.len(), 3);
    }

    #[test]
    fn test_parse_large_variable_numbers() {
        let content = "p cnf 1000 1\n999 -1000 500 0\n";
        let file = write_temp_cnf(content);

        let instance = parse(file.path().to_str().unwrap()).expect("Failed to parse");

        assert_eq!(instance.clauses.len(), 1);
        assert_eq!(instance.clauses[0].len(), 3);
    }

    #[test]
    fn test_parse_invalid_literal_fails() {
        let content = "p cnf 2 1\n1 abc 0\n";
        let file = write_temp_cnf(content);

        let result = parse(file.path().to_str().unwrap());

        assert!(result.is_err());
        match result {
            Err(DimacsError::ParseError(_)) => (),
            _ => panic!("Expected ParseError"),
        }
    }

    #[test]
    fn test_variable_registry_populated() {
        let content = "p cnf 5 2\n1 3 5 0\n2 4 0\n";
        let file = write_temp_cnf(content);

        let instance = parse(file.path().to_str().unwrap()).expect("Failed to parse");

        // Should have registered 5 unique variables
        assert_eq!(instance.variables.count(), 5);
    }

    // Integration tests with real benchmark files
    #[test]
    fn test_parse_real_sat_file() {
        let instance = parse("examples/problem_specs/sat/aim-50-1_6-yes1-4.cnf")
            .expect("Failed to parse SAT benchmark");

        // From header: p cnf 50 80
        assert_eq!(instance.variables.count(), 50);
        assert_eq!(instance.clauses.len(), 80);
    }

    #[test]
    fn test_parse_real_unsat_file() {
        let instance = parse("examples/problem_specs/unsat/dubois20.cnf")
            .expect("Failed to parse UNSAT benchmark");

        // dubois20 should have specific structure
        assert!(!instance.clauses.is_empty());
        assert!(!instance.variables.iter().next().is_none());
    }

    #[test]
    fn test_parse_logistics_file() {
        let instance = parse("examples/problem_specs/sat/logistics.a.cnf")
            .expect("Failed to parse logistics benchmark");

        assert!(!instance.clauses.is_empty());
    }
}
