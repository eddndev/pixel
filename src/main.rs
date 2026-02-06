use clap::Parser;
use image::{GenericImageView, Pixel};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the input image
    #[arg(short, long)]
    input: PathBuf,

    /// Pixel block size
    #[arg(short, long, default_value_t = 10)]
    block_size: u32,
}

#[derive(Serialize)]
struct Output {
    matrix: Vec<Vec<u32>>,
    colors: HashMap<u32, String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let img = image::open(&args.input)?;
    let (width, height) = img.dimensions();
    let block_size = args.block_size;

    if block_size == 0 {
        eprintln!("Error: Block size must be greater than 0");
        std::process::exit(1);
    }

    let mut matrix: Vec<Vec<u32>> = Vec::new();
    let mut color_to_id: HashMap<String, u32> = HashMap::new();
    let mut id_to_color: HashMap<u32, String> = HashMap::new();
    let mut next_id = 1;

    for y in (0..height).step_by(block_size as usize) {
        let mut row: Vec<u32> = Vec::new();
        for x in (0..width).step_by(block_size as usize) {
            // Calculate average color for the block
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

            if count == 0 {
                continue;
            }

            let r = (r_sum / count) as u8;
            let g = (g_sum / count) as u8;
            let b = (b_sum / count) as u8;

            let hex_color = format!("#{:02x}{:02x}{:02x}", r, g, b);

            let id = if let Some(&id) = color_to_id.get(&hex_color) {
                id
            } else {
                let id = next_id;
                next_id += 1;
                color_to_id.insert(hex_color.clone(), id);
                id_to_color.insert(id, hex_color);
                id
            };

            row.push(id);
        }
        matrix.push(row);
    }

    let output = Output {
        matrix,
        colors: id_to_color,
    };

    let json_output = serde_json::to_string_pretty(&output)?;
    println!("{}", json_output);

    Ok(())
}