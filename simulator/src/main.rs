use clap::Parser;

mod encoder;
mod simulator;

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long, default_value = "instrs.rle")]
    input: String,

    #[arg(short, long, default_value = "")]
    output: String,

    #[arg(short, long)]
    video: bool,

    #[arg(short = 'd', long)]
    video_debug: bool,

    #[arg(short = 'I', long)]
    image: bool,

    #[arg(short, long, default_value_t = 1)]
    framestep: usize,
}

fn main() -> Result<(), String> {
    let args = Args::parse();

    if args.video {
        let outpath = if args.output.is_empty() {
            "output.gif"
        } else {
            &args.output
        };

        simulator::reconstruct_video(&args.input, outpath, args.framestep, args.video_debug)?;
        println!("Saved animation to {}", outpath);
    } else if args.image {
        let outpath = if args.output.is_empty() {
            "output.png"
        } else {
            &args.output
        };

        simulator::reconstruct_image(&args.input, outpath)?;
        println!("Saved image to {}", outpath);
    }

    Ok(())
}
