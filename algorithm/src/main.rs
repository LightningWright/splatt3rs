use std::{collections::HashSet, fs::File, hash::Hash, io::Write, num::NonZero};

use clap::Parser;

use algorithm::Op;

use crate::algorithms::AlgorithmSettings;

mod algorithms;
mod rle;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
struct Point(i32, i32);

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, default_value = "")]
    input: String,

    #[arg(short, long, default_value = "instrs.rle")]
    output: String,

    #[arg(short, long)]
    fast: bool,
}

fn main() {
    let args = Args::parse();

    let workers = std::thread::available_parallelism()
        .unwrap_or(NonZero::new(16usize).unwrap())
        .get();

    let settings = AlgorithmSettings {
        retries: if args.fast { 128 } else { 8192 },
        workers,
    };

    let img = image::open(args.input).expect("failed to load image");
    let pixels = load_binary(&img);

    println!("Pixels to print {}", pixels.len());

    let instrs = algorithms::run_algorithms(pixels, settings);

    let mut file = File::create(args.output).unwrap();
    let encoded_instrs = rle::rle_encode(&instrs);

    file.write_all(&encoded_instrs).unwrap();
    println!("Saved instructions file");
}

fn load_binary(img: &image::DynamicImage) -> HashSet<Point> {
    let gray = img.to_luma8();
    let mut set = HashSet::new();

    for y in 0..gray.height() {
        for x in 0..gray.width() {
            if gray.get_pixel(x, y).0[0] == 0 {
                set.insert(Point(x as i32, y as i32));
            }
        }
    }
    set
}
