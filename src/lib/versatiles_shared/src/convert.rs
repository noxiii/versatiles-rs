use crate::Result;

use super::{compress::*, image::*, Blob, Precompression};
use clap::ValueEnum;
use std::fmt::Debug;

/// A structure representing a function that converts a blob to another blob
struct FnConv {
	func: fn(Blob) -> Result<Blob>,
	name: String,
}

impl FnConv {
	/// Create a new `FnConv` from a function and a name
	fn new(func: fn(Blob) -> Result<Blob>, name: &str) -> FnConv {
		FnConv {
			func,
			name: name.to_owned(),
		}
	}

	/// Create an optional `FnConv` from a function and a name
	fn some(func: fn(Blob) -> Result<Blob>, name: &str) -> Option<FnConv> {
		Some(FnConv::new(func, name))
	}
}

impl Debug for FnConv {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("FnConv")
			.field("func", &self.func)
			.field("name", &self.name)
			.finish()
	}
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum TileFormat {
	BIN,
	PNG,
	JPG,
	WEBP,
	AVIF,
	SVG,
	PBF,
	GEOJSON,
	TOPOJSON,
	JSON,
}

/// A structure representing a pipeline of conversions to be applied to a blob
#[derive(Debug)]
pub struct DataConverter {
	pipeline: Vec<FnConv>,
}

impl DataConverter {
	/// Create a new empty `DataConverter`
	pub fn new_empty() -> DataConverter {
		DataConverter { pipeline: Vec::new() }
	}

	/// Return `true` if the `DataConverter` has an empty pipeline
	pub fn is_empty(&self) -> bool {
		self.pipeline.is_empty()
	}

	/// Create a new `DataConverter` for tile recompression from `src_form` and `src_comp` to `dst_form` and `dst_comp`
	/// with optional forced recompression
	pub fn new_tile_recompressor(
		src_form: &TileFormat, src_comp: &Precompression, dst_form: &TileFormat, dst_comp: &Precompression,
		force_recompress: bool,
	) -> DataConverter {
		let mut converter = DataConverter::new_empty();

		// Create a format converter function based on the source and destination formats.
		let format_converter_option: Option<FnConv> = if (src_form != dst_form) || force_recompress {
			use TileFormat::*;
			match (src_form, dst_form) {
				(PNG, JPG) => FnConv::some(|tile| -> Result<Blob> { img2jpg(&png2img(tile)?) }, "PNG->JPG"),
				(PNG, PNG) => FnConv::some(|tile| -> Result<Blob> { img2png(&png2img(tile)?) }, "PNG->PNG"),
				(PNG, WEBP) => FnConv::some(
					|tile| -> Result<Blob> { img2webplossless(&png2img(tile)?) },
					"PNG->WEBP",
				),

				(JPG, PNG) => FnConv::some(|tile| -> Result<Blob> { img2png(&jpg2img(tile)?) }, "JPG->PNG"),
				(JPG, WEBP) => FnConv::some(|tile| -> Result<Blob> { img2webp(&jpg2img(tile)?) }, "JPG->WEBP"),

				(WEBP, JPG) => FnConv::some(|tile| -> Result<Blob> { img2jpg(&webp2img(tile)?) }, "WEBP->JPG"),
				(WEBP, PNG) => FnConv::some(|tile| -> Result<Blob> { img2png(&webp2img(tile)?) }, "WEBP->PNG"),

				(_, _) => {
					if src_form == dst_form {
						None
					} else {
						todo!("convert {:?} -> {:?}", src_form, dst_form)
					}
				}
			}
		} else {
			None
		};

		// Push the necessary conversion functions to the converter pipeline.
		if (src_comp == dst_comp) && !force_recompress {
			if let Some(format_converter) = format_converter_option {
				converter.push(format_converter)
			}
		} else {
			use Precompression::*;
			match src_comp {
				Uncompressed => {}
				Gzip => converter.push(FnConv::new(decompress_gzip, "decompress_gzip")),
				Brotli => converter.push(FnConv::new(decompress_brotli, "decompress_brotli")),
			}
			if let Some(format_converter) = format_converter_option {
				converter.push(format_converter)
			}
			match dst_comp {
				Uncompressed => {}
				Gzip => converter.push(FnConv::new(compress_gzip, "compress_gzip")),
				Brotli => converter.push(FnConv::new(compress_brotli, "compress_brotli")),
			}
		};

		converter
	}
	/// Constructs a new `DataConverter` instance that compresses data using the specified precompression algorithm.
	/// The `dst_comp` parameter specifies the precompression algorithm to use: `Precompression::Uncompressed`, `Precompression::Gzip`, or `Precompression::Brotli`.
	pub fn new_compressor(dst_comp: &Precompression) -> DataConverter {
		let mut converter = DataConverter::new_empty();

		match dst_comp {
			// If uncompressed, do nothing
			Precompression::Uncompressed => {}
			// If gzip, add the gzip compression function to the pipeline
			Precompression::Gzip => converter.push(FnConv::new(compress_gzip, "compress_gzip")),
			// If brotli, add the brotli compression function to the pipeline
			Precompression::Brotli => converter.push(FnConv::new(compress_brotli, "compress_brotli")),
		}

		converter
	}

	/// Constructs a new `DataConverter` instance that decompresses data using the specified precompression algorithm.
	/// The `src_comp` parameter specifies the precompression algorithm to use: `Precompression::Uncompressed`, `Precompression::Gzip`, or `Precompression::Brotli`.
	pub fn new_decompressor(src_comp: &Precompression) -> DataConverter {
		let mut converter = DataConverter::new_empty();

		match src_comp {
			// If uncompressed, do nothing
			Precompression::Uncompressed => {}
			// If gzip, add the gzip decompression function to the pipeline
			Precompression::Gzip => converter.push(FnConv::new(decompress_gzip, "decompress_gzip")),
			// If brotli, add the brotli decompression function to the pipeline
			Precompression::Brotli => converter.push(FnConv::new(decompress_brotli, "decompress_brotli")),
		}

		converter
	}
	/// Adds a new conversion function to the pipeline.
	fn push(&mut self, f: FnConv) {
		self.pipeline.push(f);
	}

	/// Runs the data through the pipeline of conversion functions and returns the result.
	pub fn run(&self, mut data: Blob) -> Result<Blob> {
		for f in self.pipeline.iter() {
			data = (f.func)(data)?;
		}
		Ok(data)
	}

	/// Returns a string describing the pipeline of conversion functions.
	pub fn description(&self) -> String {
		let names: Vec<String> = self.pipeline.iter().map(|e| e.name.clone()).collect();
		names.join(", ")
	}
}

/// Implements the `PartialEq` trait for the `DataConverter` struct.
/// This function returns true if the `description` method of both `DataConverter` instances returns the same value.
impl PartialEq for DataConverter {
	fn eq(&self, other: &Self) -> bool {
		self.description() == other.description()
	}
}

/// Implements the `Eq` trait for the `DataConverter` struct.
/// This trait is used in conjunction with `PartialEq` to provide a total equality relation for `DataConverter` instances.
impl Eq for DataConverter {}
