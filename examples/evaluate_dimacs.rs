extern crate env_logger;
extern crate itertools;
extern crate smellysat;

use itertools::Itertools;
use log::info;
use smellysat::dimacs;
use std::{env, process};

use thiserror::Error;

#[derive(Error, Debug)]
enum Error {
    #[error("failed to parse input")]
    ParsingError(#[from] dimacs::DimacsError),
}

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let filepath = match args.len() {
        2 => args.get(1).unwrap(),
        _ => {
            eprintln!("c evaluate_dimacs [path to problem file]");
            process::exit(-1);
        }
    };
    match run(filepath) {
        Err(err) => {
            eprintln!("c {}", err);
            eprintln!("c execution failed");
            process::exit(-1);
        }
        Ok(()) => return,
    }
}

fn run(filepath: &str) -> Result<(), Error> {
    let mut instance = dimacs::parse(filepath)?;

    eprintln!("c evaluating");
    let sol = instance.solve();
    match sol.assignments() {
        None => println!("s UNSATISFIABLE"),
        Some(assignment_set) => {
            println!("s SATISFIABLE");

            let mut by_var_name = assignment_set
                .iter()
                .map(|lit| (sol.literals.get(lit.var()), lit.polarity()))
                .collect::<Vec<_>>();
            by_var_name.sort_by_key(|e| e.0.parse::<u64>().unwrap());

            let formatted_vars = by_var_name.iter().map(|(var_name, polarity)| {
                format!("{}{}", if *polarity { "" } else { "-" }, var_name)
            });
            let solution =
                Itertools::intersperse(formatted_vars, " ".to_string()).collect::<String>();
            println!("v {} 0", solution);
        }
    }
    eprintln!("c stats {:?}", sol.stats);
    Ok(())
}
