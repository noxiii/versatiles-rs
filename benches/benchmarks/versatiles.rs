use crate::container::get_reader;
use crate::shared::TileCoord3;
use criterion::{black_box, criterion_group, Criterion};
use futures::executor::block_on;
use log::{set_max_level, LevelFilter};
use rand::{seq::SliceRandom, thread_rng};

fn versatiles_read(c: &mut Criterion) {
	set_max_level(LevelFilter::Warn);

	c.bench_function("get_tile_data", |b| {
		let reader = block_on(get_reader("benches/ressources/berlin.versatiles")).unwrap();
		let coords: Vec<TileCoord3> = reader
			.get_parameters()
			.get_bbox_pyramide()
			.iter_tile_indexes()
			.collect();

		b.iter(|| {
			let coord = coords.choose(&mut thread_rng()).unwrap();
			black_box(reader.get_tile_data(coord));
		})
	});
}

criterion_group!(versatiles, versatiles_read);
