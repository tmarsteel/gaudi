use std::fmt::{Debug, Display, Formatter};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::io::BufReader;
use colored::{Color, ColoredString, Colorize, Style};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba, RgbaImage};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    input_file: PathBuf,

    #[arg(long, value_enum, default_value = "up")]
    vertical_gravity: VerticalDirection,

    #[arg(long)]
    resize_to_width: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum VerticalDirection {
    UP,
    DOWN,
}

fn main() {
    let args = Args::parse();

    let input_file = std::fs::File::open(&args.input_file).unwrap_or_else(|e| panic!("Could not open file {}: {}", args.input_file.display(), e));
    let decoder = png::Decoder::new(BufReader::new(input_file));
    let mut reader = decoder.read_info().unwrap_or_else(|e| panic!("Failed to read PNG data from {}: {}", args.input_file.display(), e));
    let info = reader.info().clone();
    let mut image_buffer = vec![0; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut image_buffer).unwrap_or_else(|e| panic!("Failed to read PNG data from {}: {}", args.input_file.display(), e));
    drop(reader);

    let mut image = DynamicImage::ImageRgba8(ImageBuffer::from_vec(info.width, info.height, image_buffer).unwrap());
    if let Some(resize_to_width) = args.resize_to_width {
        let factor = resize_to_width as f32 / image.width() as f32;
        let new_height = (image.height() as f32 * factor) as u32;
        image = image.resize(resize_to_width, new_height, image::imageops::FilterType::Nearest);
    }

    let slices = image_to_ascii(image, args.vertical_gravity, color_mapping_truecolor);
    slices.into_iter().for_each(|s| print!("{}", s));
}

fn image_to_ascii(
    image: DynamicImage,
    vertical_gravity: VerticalDirection,
    color_mapper: fn(&Rgba<u8>) -> Color,
) -> Vec<ColoredString> {
    let mut as_string: Vec<ColoredString> = Vec::with_capacity((image.width() as usize + 1) * (image.height() as usize / 2 + 1));
    let mut row: u32 = 0;
    if image.height() % 2 != 0 && vertical_gravity == VerticalDirection::DOWN {
        for col in 0..image.width() {
            let upper_pixel = Rgba::from([0, 0, 0, 0]);
            let lower_pixel = image.get_pixel(col, 0);
            as_string.push(two_pixels_to_ascii_char(&upper_pixel, &lower_pixel, color_mapper));
        }
        as_string.push("\n".clear());
        row = 1;
    }

    loop {
        if row + 1 >= image.height() {
            break;
        }
        for col in 0..image.width() {
            let upper_pixel = image.get_pixel(col, row);
            let lower_pixel = image.get_pixel(col, row + 1);
            as_string.push(two_pixels_to_ascii_char(&upper_pixel, &lower_pixel, color_mapper));
        }
        as_string.push("\n".clear());
        row += 2;
    }

    if image.height() % 2 != 0 && vertical_gravity == VerticalDirection::UP {
        for col in 0..image.width() {
            let upper_pixel = image.get_pixel(col, image.height() - 1);
            let lower_pixel = Rgba::from([0, 0, 0, 0]);
            as_string.push(two_pixels_to_ascii_char(&upper_pixel, &lower_pixel, color_mapper));
        }
        as_string.push("\n".clear());
    }

    as_string
}

fn is_transparent(pixel: &Rgba<u8>) -> bool {
    pixel[3] == 0
}

fn two_pixels_to_ascii_char(
    upper_pixel: &Rgba<u8>,
    lower_pixel: &Rgba<u8>,
    color_mapper: fn (&Rgba<u8>) -> Color,
) -> ColoredString {
    if is_transparent(upper_pixel) && is_transparent(lower_pixel) {
        return " ".clear()
    }

    if is_transparent(upper_pixel) {
        assert!(!is_transparent(lower_pixel));
        return "▄".color(color_mapper(lower_pixel))
    }

    if is_transparent(lower_pixel) {
        assert!(!is_transparent(upper_pixel));
        return "▀".color(color_mapper(upper_pixel))
    }

    "▄".color(color_mapper(lower_pixel)).on_color(color_mapper(upper_pixel))
}

fn color_mapping_truecolor(pixel: &Rgba<u8>) -> Color {
    Color::TrueColor {
        r: pixel[0],
        g: pixel[1],
        b: pixel[2],
    }
}