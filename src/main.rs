use clap::Parser;
use std::path::PathBuf;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// The path to the DTED file
    #[arg(short, long)]
    path: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    match spdted::DtedTile::from_file(&cli.path) {
        Ok(tile) => {
            println!("DTED Tile loaded successfully!");
            println!("Origin: {:?}", tile.header().origin());
            println!("Number of lat points: {}", tile.header().num_lat());
            println!("Number of lon points: {}", tile.header().num_lon());
        }
        Err(e) => {
            eprintln!("Failed to load DTED Tile: {}", e);
        }
    }
}
