extern crate smellysat;

use std::collections::HashMap;

use smellysat::problem_builder::{BoolExpr, ProblemBuilder};

fn main() {
    let mut pb = ProblemBuilder::new();

    let children = vec!["laurie", "lucy", "eric", "rita"];
    let seats = vec!["a", "b", "c", "d"];

    let mut by_child = HashMap::new();
    let mut by_seat = HashMap::new();

    for child in children.iter() {
        for seat in seats.iter() {
            let var = pb.var(&format!("{}X{}", &child, &seat));
            by_child
                .entry(child)
                .or_insert(HashMap::new())
                .insert(seat, var);
            by_seat
                .entry(seat)
                .or_insert(HashMap::new())
                .insert(child, var);
        }
    }
    // everyone needs a seat
    for child in &children {
        pb.require(or_list(
            &pb,
            &by_child[child]
                .values()
                .map(|x| *x)
                .collect::<Vec<BoolExpr>>(),
        ))
    }

    // two children can not be in the same seat
    for seat in &seats {
        let mut constraints = vec![];
        for chosen_child in &children {
            let others = children
                .iter()
                .filter(|&name| name != chosen_child)
                .map(|name| by_seat[seat][name])
                .map(|var| pb.not(var))
                .collect::<Vec<_>>();
            constraints.push(and_list(&pb, &others));
        }
        pb.require(or_list(&pb, &constraints));
    }

    let mut instance = pb.build();
    let result = instance.solve();
    println!("{:?}", result);
}

fn or_list(pb: &ProblemBuilder, xs: &Vec<BoolExpr>) -> BoolExpr {
    match xs.len() {
        0 => panic!("Cannot or empty list"),
        1 => xs[0],
        _ => {
            let mut acc = xs[0];
            for i in 1..xs.len() {
                acc = pb.or(acc, xs[i])
            }
            acc
        }
    }
}

fn and_list(pb: &ProblemBuilder, xs: &Vec<BoolExpr>) -> BoolExpr {
    match xs.len() {
        0 => panic!("Cannot or empty list"),
        1 => xs[0],
        _ => {
            let mut acc = xs[0];
            for i in 1..xs.len() {
                acc = pb.and(acc, xs[i])
            }
            acc
        }
    }
}
