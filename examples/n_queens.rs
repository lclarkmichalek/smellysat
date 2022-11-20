extern crate smellysat;

use std::{
    iter::{Filter, StepBy},
    ops::Range,
};

use smellysat::problem_builder::ProblemBuilder;

fn main() {
    println!("Hello, world!");
    let mut pb = ProblemBuilder::new();

    let grid = (0..64)
        .map(|ix| pb.var(&format!("{}x{}", ix / 8, ix % 8)))
        .collect::<Vec<_>>();

    // 1 per row
    for row_ix in 0..8 {
        let cell_ixs = (row_ix * 8)..(row_ix * 8 + 8);

        println!("{:?}", cell_ixs.clone().collect::<Vec<_>>());
        let mut constraints = vec![];
        for entry in cell_ixs.clone() {
            // ensure all the others are unset, and this one is set
            let others_unset = cell_ixs
                .clone()
                .filter(|ix| *ix != entry)
                .map(|ix| pb.not(grid[ix]))
                .reduce(|a, b| pb.and(a, b))
                .unwrap();
            constraints.push(pb.and(grid[entry], others_unset));
        }
        // Or, no queens in the row
        constraints.push(
            cell_ixs
                .clone()
                .map(|ix| pb.not(grid[ix]))
                .reduce(|a, b| pb.and(a, b))
                .unwrap(),
        );
        pb.require(
            constraints
                .iter()
                .map(|x| *x)
                .reduce(|a, b| pb.or(a, b))
                .unwrap(),
        )
    }

    // 1 per col
    for col_ix in 0..8 {
        let cell_ixs = (col_ix..64).step_by(8);
        println!("{:?}", cell_ixs.clone().collect::<Vec<_>>());
        let mut constraints = vec![];
        for entry in cell_ixs.clone() {
            // ensure all the others are unset, and this one is set
            let others_unset = cell_ixs
                .clone()
                .filter(|ix| *ix != entry)
                .map(|ix| pb.not(grid[ix]))
                .reduce(|a, b| pb.and(a, b))
                .unwrap();
            constraints.push(pb.and(grid[entry], others_unset));
        }
        // Or, no queens in the col
        constraints.push(
            cell_ixs
                .clone()
                .map(|ix| pb.not(grid[ix]))
                .reduce(|a, b| pb.and(a, b))
                .unwrap(),
        );
        pb.require(constraints.into_iter().reduce(|a, b| pb.or(a, b)).unwrap())
    }

    // Diagonals, l->r
    for col_ix in (-8)..8 {
        let cell_ixs: Filter<StepBy<Range<i64>>, _> =
            (col_ix..128).step_by(9).filter(|ix| 0 <= *ix && *ix < 64);
        println!("diag: {:?}", cell_ixs.clone().collect::<Vec<_>>());
        let mut constraints = vec![];
        for entry in cell_ixs.clone() {
            // ensure all the others are unset, and this one is set
            let others_unset = cell_ixs
                .clone()
                .filter(|&ix| ix != entry)
                .map(|ix| pb.not(grid[ix as usize]))
                .reduce(|a, b| pb.and(a, b))
                .unwrap();
            constraints.push(pb.and(grid[entry as usize], others_unset))
        }
        // or zero
        constraints.push(
            cell_ixs
                .clone()
                .map(|ix| pb.not(grid[ix as usize]))
                .reduce(|a, b| pb.and(a, b))
                .unwrap(),
        );
        pb.require(constraints.into_iter().reduce(|a, b| pb.or(a, b)).unwrap());
    }

    // for col_ix in 0..16 {
    //     let cell_ixs = (col_ix..128).step_by(7).filter(|ix| 0 <= *ix && *ix < 64);
    //     for entry in cell_ixs.clone() {
    //         // ensure all the others are unset, and this one is set
    //         let others_unset = cell_ixs
    //             .clone()
    //             .filter(|&ix| ix != entry)
    //             .map(|ix| grid[ix])
    //             .reduce(|a, b| pb.and(pb.not(a), pb.not(b)))
    //             .unwrap();
    //         pb.require(pb.and(grid[entry as usize], others_unset));
    //     }
    // }

    // let cell_ixs = (0..64);
    // for entry in cell_ixs.clone() {
    //     let others_unset = cell_ixs
    //         .clone()
    //         .map(|ix| grid[ix])
    //         .reduce(|a, b| pb.and(pb.not(a), pb.not(b)))
    //         .unwrap();
    //     pb.require(pb.and(grid[entry], others_unset));
    // }

    let mut instance = pb.build();
    let result = instance.solve();
    println!("{:?}", result);
}
