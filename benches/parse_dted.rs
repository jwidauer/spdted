use criterion::{criterion_group, criterion_main, Criterion};
use std::path::Path;

fn benchmark(c: &mut Criterion) {
    let resource_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
    let path = resource_dir.join("n47.dt2");
    let bytes = std::fs::read(path).unwrap();

    c.bench_function("parse_tile", |b| {
        b.iter(|| spdted::DtedTile::from_bytes(&bytes).unwrap())
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(6000)
        .nresamples(700_000)
        .measurement_time(std::time::Duration::from_secs(30))
        .without_plots();
    targets = benchmark
}
criterion_main!(benches);

// fn iai_bench() {
//     let resource_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
//     let path = resource_dir.join("n47.dt2");
//     let bytes = std::fs::read(path).unwrap();

//     for _ in 0..1000 {
//         let _ = spdted::DtedTile::from_bytes(&bytes).unwrap();
//     }
// }

// iai::main!(iai_bench);
