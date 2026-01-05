use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use smellysat::dimacs;

fn benchmark_sat_problems(c: &mut Criterion) {
    let mut group = c.benchmark_group("sat");

    // Small SAT problem - aim-50
    let aim50_path = "examples/problem_specs/sat/aim-50-1_6-yes1-4.cnf";
    if std::path::Path::new(aim50_path).exists() {
        group.bench_function("aim-50-1_6-yes1-4", |b| {
            b.iter(|| {
                let mut instance = dimacs::parse(black_box(aim50_path)).unwrap();
                let solution = instance.solve();
                assert!(solution.assignments().is_some());
                solution
            })
        });
    }

    // Logistics problems of varying sizes
    for name in ["logistics.a", "logistics.b", "logistics.c", "logistics.d"] {
        let path = format!("examples/problem_specs/sat/{}.cnf", name);
        if std::path::Path::new(&path).exists() {
            group.bench_function(name, |b| {
                b.iter(|| {
                    let mut instance = dimacs::parse(black_box(&path)).unwrap();
                    let solution = instance.solve();
                    assert!(solution.assignments().is_some());
                    solution
                })
            });
        }
    }

    group.finish();
}

fn benchmark_unsat_problems(c: &mut Criterion) {
    let mut group = c.benchmark_group("unsat");

    // Benchmark dubois problems of increasing size
    // These are known hard UNSAT instances
    for size in [20, 21, 22, 23, 24, 25] {
        let path = format!("examples/problem_specs/unsat/dubois{}.cnf", size);
        if std::path::Path::new(&path).exists() {
            group.bench_function(BenchmarkId::new("dubois", size), |b| {
                b.iter(|| {
                    let mut instance = dimacs::parse(black_box(&path)).unwrap();
                    let solution = instance.solve();
                    assert!(solution.assignments().is_none());
                    solution
                })
            });
        }
    }

    // aim-100 UNSAT instance
    let aim100_path = "examples/problem_specs/unsat/aim-100-1_6-no-1.cnf";
    if std::path::Path::new(aim100_path).exists() {
        group.bench_function("aim-100-1_6-no-1", |b| {
            b.iter(|| {
                let mut instance = dimacs::parse(black_box(aim100_path)).unwrap();
                let solution = instance.solve();
                assert!(solution.assignments().is_none());
                solution
            })
        });
    }

    group.finish();
}

fn benchmark_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("parsing");

    // Benchmark parsing different file sizes
    let files = [
        ("aim-50", "examples/problem_specs/sat/aim-50-1_6-yes1-4.cnf"),
        (
            "aim-100",
            "examples/problem_specs/unsat/aim-100-1_6-no-1.cnf",
        ),
        ("logistics.a", "examples/problem_specs/sat/logistics.a.cnf"),
        ("dubois50", "examples/problem_specs/unsat/dubois50.cnf"),
    ];

    for (name, path) in files {
        if std::path::Path::new(path).exists() {
            // Set throughput based on file size
            let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            group.throughput(Throughput::Bytes(file_size));

            group.bench_function(name, |b| b.iter(|| dimacs::parse(black_box(path)).unwrap()));
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_sat_problems,
    benchmark_unsat_problems,
    benchmark_parsing
);
criterion_main!(benches);
