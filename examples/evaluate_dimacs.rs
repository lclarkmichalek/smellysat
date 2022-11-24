extern crate itertools;
extern crate smellysat;

use itertools::Itertools;
use smellysat::dimacs;
use std::{env, process};

use thiserror::Error;

#[derive(Error, Debug)]
enum Error {
    #[error("failed to parse input")]
    ParsingError(#[from] dimacs::DimacsError),
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let filepath = match args.len() {
        2 => args.get(1).unwrap(),
        _ => {
            eprintln!("evaluate_dimacs [path to problem file]");
            process::exit(-1);
        }
    };
    match run(filepath) {
        Err(err) => {
            eprintln!("{}", err);
            eprintln!("execution failed");
            process::exit(-1);
        }
        Ok(()) => return,
    }
}

fn run(filepath: &str) -> Result<(), Error> {
    let mut instance = dimacs::parse(filepath)?;

    eprintln!("evaluating");
    let sol = instance.solve();
    match sol.assignments() {
        None => println!("s UNSATISFIABLE"),
        Some(mut assignments) => {
            println!("s SATISFIABLE");
            assignments.sort_by_key(|e| e.var().0);
            let solution = assignments
                .iter()
                .map(|lit| format!("{}{}", if lit.polarity() { "" } else { "-" }, lit.var().0))
                .intersperse(" ".to_string())
                .collect::<String>();
            println!("v {} 0", solution);
        }
    }
    Ok(())
}
