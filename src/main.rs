use clap::{Parser, Subcommand};
use image::{GenericImageView, Pixel};
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Pixelate an image with a specific block size
    Pixelate {
        /// Path to the input image
        #[arg(short, long)]
        input: PathBuf,

        /// Pixel block size
        #[arg(short, long, default_value_t = 10)]
        block_size: u32,

        /// Optional path to output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Map every single pixel of the image to its color ID
    Map {
        /// Path to the input image
        #[arg(short, long)]
        input: PathBuf,

        /// Optional path to output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Serialize)]
struct Output {
    matrix: Vec<Vec<u32>>,
    colors: HashMap<u32, String>,
}

fn process_image(input_path: &PathBuf, block_size: u32, output_path: Option<&PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let img = image::open(input_path)?;
    let (width, height) = img.dimensions();

    let mut matrix: Vec<Vec<u32>> = Vec::new();
    let mut color_to_id: HashMap<String, u32> = HashMap::new();
    let mut id_to_color: HashMap<u32, String> = HashMap::new();
    let mut next_id = 1;

    for y in (0..height).step_by(block_size as usize) {
        let mut row: Vec<u32> = Vec::new();
        for x in (0..width).step_by(block_size as usize) {
            let r: u8;
            let g: u8;
            let b: u8;

            if block_size > 1 {
                let mut r_sum: u64 = 0;
                let mut g_sum: u64 = 0;
                let mut b_sum: u64 = 0;
                let mut count: u64 = 0;

                let x_end = (x + block_size).min(width);
                let y_end = (y + block_size).min(height);

                for by in y..y_end {
                    for bx in x..x_end {
                        let pixel = img.get_pixel(bx, by);
                        let rgb = pixel.to_rgb();
                        r_sum += rgb[0] as u64;
                        g_sum += rgb[1] as u64;
                        b_sum += rgb[2] as u64;
                        count += 1;
                    }
                }
                r = (r_sum / count) as u8;
                g = (g_sum / count) as u8;
                b = (b_sum / count) as u8;
            } else {
                let pixel = img.get_pixel(x, y);
                let rgb = pixel.to_rgb();
                r = rgb[0];
                g = rgb[1];
                b = rgb[2];
            }

            let hex_color = format!("#{:02x}{:02x}{:02x}", r, g, b);

            let id = *color_to_id.entry(hex_color.clone()).or_insert_with(|| {
                let id = next_id;
                id_to_color.insert(id, hex_color);
                next_id += 1;
                id
            });

            row.push(id);
        }
        matrix.push(row);
    }

    let output = Output {
        matrix,
        colors: id_to_color,
    };

    let json_output = serde_json::to_string_pretty(&output)?;

    if let Some(path) = output_path {
        let mut file = File::create(path)?;
        file.write_all(json_output.as_bytes())?;
    } else {
        println!("{}", json_output);
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Pixelate { input, block_size, output } => {
            if *block_size == 0 {
                eprintln!("Error: Block size must be greater than 0");
                std::process::exit(1);
            }
            process_image(input, *block_size, output.as_ref())
        }
        Commands::Map { input, output } => process_image(input, 1, output.as_ref()),
    }
}
