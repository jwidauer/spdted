use criterion::{criterion_group, criterion_main, Criterion};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

fn benchmark(c: &mut Criterion) {
    let resource_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
    let path = resource_dir.join("n47.dt2");
    let file = File::open(path).unwrap();
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer).unwrap();

    c.bench_function("parse_tile", |b| {
        b.iter(|| spdted::DtedTile::from_bytes(&buffer).unwrap())
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(3000)
        .nresamples(600_000)
        .measurement_time(std::time::Duration::from_secs(15))
        .without_plots();
    targets = benchmark
}
criterion_main!(benches);
