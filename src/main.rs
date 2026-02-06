use clap::{Parser, Subcommand};
use image::{GenericImageView, ImageBuffer, Pixel, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
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

        /// Color grouping tolerance (0.0 to ~510.0)
        #[arg(short, long, default_value_t = 0.0)]
        tolerance: f64,
    },
    /// Map every single pixel of the image to its color ID
    Map {
        /// Path to the input image
        #[arg(short, long)]
        input: PathBuf,

        /// Optional path to output file
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Color grouping tolerance (0.0 to ~510.0)
        #[arg(short, long, default_value_t = 0.0)]
        tolerance: f64,
    },
    /// Reconstruct an image from a JSON output file
    Reconstruct {
        /// Path to the input JSON file
        #[arg(short, long)]
        input: PathBuf,

        /// Path to the output image
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Serialize, Deserialize)]
struct Output {
    matrix: Vec<Vec<u32>>,
    colors: HashMap<u32, String>,
}

fn color_distance(c1: &Rgba<u8>, c2: &Rgba<u8>) -> f64 {
    let r_diff = c1[0] as f64 - c2[0] as f64;
    let g_diff = c1[1] as f64 - c2[1] as f64;
    let b_diff = c1[2] as f64 - c2[2] as f64;
    let a_diff = c1[3] as f64 - c2[3] as f64;
    (r_diff * r_diff + g_diff * g_diff + b_diff * b_diff + a_diff * a_diff).sqrt()
}

fn process_image(input_path: &PathBuf, block_size: u32, output_path: Option<&PathBuf>, tolerance: f64) -> Result<(), Box<dyn std::error::Error>> {
    let img = image::open(input_path)?;
    let (width, height) = img.dimensions();

    let mut matrix: Vec<Vec<u32>> = Vec::new();
    let mut color_to_id: HashMap<String, u32> = HashMap::new();
    let mut id_to_color: HashMap<u32, String> = HashMap::new();
    // Cache of canonical colors for fuzzy matching: (ID, RGBA)
    let mut palette: Vec<(u32, Rgba<u8>)> = Vec::new();

    // Reserve ID 0 for fully transparent
    let transparent_hex = "#00000000".to_string();
    color_to_id.insert(transparent_hex.clone(), 0);
    id_to_color.insert(0, transparent_hex);

    let mut next_id = 1;

    for y in (0..height).step_by(block_size as usize) {
        let mut row: Vec<u32> = Vec::new();
        for x in (0..width).step_by(block_size as usize) {
            let r: u8;
            let g: u8;
            let b: u8;
            let a: u8;

            if block_size > 1 {
                let mut r_sum: u64 = 0;
                let mut g_sum: u64 = 0;
                let mut b_sum: u64 = 0;
                let mut a_sum: u64 = 0;
                let mut count: u64 = 0;

                let x_end = (x + block_size).min(width);
                let y_end = (y + block_size).min(height);

                for by in y..y_end {
                    for bx in x..x_end {
                        let pixel = img.get_pixel(bx, by);
                        let rgba = pixel.to_rgba();
                        r_sum += rgba[0] as u64;
                        g_sum += rgba[1] as u64;
                        b_sum += rgba[2] as u64;
                        a_sum += rgba[3] as u64;
                        count += 1;
                    }
                }
                
                let avg_a = (a_sum / count) as u8;
                if avg_a == 0 {
                    r = 0;
                    g = 0;
                    b = 0;
                    a = 0;
                } else {
                    r = (r_sum / count) as u8;
                    g = (g_sum / count) as u8;
                    b = (b_sum / count) as u8;
                    a = avg_a;
                }
            } else {
                let pixel = img.get_pixel(x, y);
                let rgba = pixel.to_rgba();
                if rgba[3] == 0 {
                    r = 0;
                    g = 0;
                    b = 0;
                    a = 0;
                } else {
                    r = rgba[0];
                    g = rgba[1];
                    b = rgba[2];
                    a = rgba[3];
                }
            }

            let current_rgba = Rgba([r, g, b, a]);
            let hex_color = format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a);

            // 1. Try exact match
            let id = if let Some(&existing_id) = color_to_id.get(&hex_color) {
                existing_id
            } else {
                // 2. Try fuzzy match (if tolerance > 0 and not transparent)
                let mut found_id = None;
                if tolerance > 0.0 && a > 0 {
                    for (pid, p_color) in &palette {
                        if color_distance(&current_rgba, p_color) <= tolerance {
                            found_id = Some(*pid);
                            break;
                        }
                    }
                }

                if let Some(fid) = found_id {
                    // Map this specific slightly-different hex to the existing ID for future speed
                    color_to_id.insert(hex_color.clone(), fid);
                    fid
                } else {
                    // New color
                    let id = next_id;
                    if id != 0 { // Should always be true as we start at 1
                         palette.push((id, current_rgba));
                    }
                    color_to_id.insert(hex_color.clone(), id);
                    id_to_color.insert(id, hex_color);
                    next_id += 1;
                    id
                }
            };

            row.push(id);
        }
        matrix.push(row);
    }

    let output = Output {
        matrix,
        colors: id_to_color,
    };

    // Custom JSON serialization to keep matrix rows on single lines
    let mut json_output = String::new();
    json_output.push_str("{\n  \"matrix\": [\n");
    for (i, row) in output.matrix.iter().enumerate() {
        let row_str = serde_json::to_string(row)?;
        json_output.push_str("    ");
        json_output.push_str(&row_str);
        if i < output.matrix.len() - 1 {
            json_output.push_str(",");
        }
        json_output.push_str("\n");
    }
    json_output.push_str("  ],\n  \"colors\": ");
    let colors_json = serde_json::to_string_pretty(&output.colors)?;
    json_output.push_str(&colors_json);
    json_output.push_str("\n}");

    if let Some(path) = output_path {
        let mut file = File::create(path)?;
        file.write_all(json_output.as_bytes())?;
    } else {
        println!("{}", json_output);
    }

    Ok(())
}

fn hex_to_rgba(hex: &str) -> Result<Rgba<u8>, String> {
    if hex.len() != 9 || !hex.starts_with('#') {
        return Err(format!("Invalid hex color: {}", hex));
    }
    let r = u8::from_str_radix(&hex[1..3], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&hex[3..5], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&hex[5..7], 16).map_err(|e| e.to_string())?;
    let a = u8::from_str_radix(&hex[7..9], 16).map_err(|e| e.to_string())?;
    Ok(Rgba([r, g, b, a]))
}

fn reconstruct_image(input_path: &PathBuf, output_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::open(input_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let data: Output = serde_json::from_str(&contents)?;

    if data.matrix.is_empty() {
        return Err("Matrix is empty".into());
    }

    let height = data.matrix.len() as u32;
    let width = data.matrix[0].len() as u32;

    let mut img: RgbaImage = ImageBuffer::new(width, height);

    for (y, row) in data.matrix.iter().enumerate() {
        for (x, &id) in row.iter().enumerate() {
            if let Some(hex_color) = data.colors.get(&id) {
                let rgba = hex_to_rgba(hex_color)?;
                img.put_pixel(x as u32, y as u32, rgba);
            } else {
                eprintln!("Warning: Color ID {} not found in map", id);
                img.put_pixel(x as u32, y as u32, Rgba([0, 0, 0, 0])); // Default to transparent
            }
        }
    }

    img.save(output_path)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Pixelate { input, block_size, output, tolerance } => {
            if *block_size == 0 {
                eprintln!("Error: Block size must be greater than 0");
                std::process::exit(1);
            }
            process_image(input, *block_size, output.as_ref(), *tolerance)
        }
        Commands::Map { input, output, tolerance } => process_image(input, 1, output.as_ref(), *tolerance),
        Commands::Reconstruct { input, output } => reconstruct_image(input, output),
    }
}
