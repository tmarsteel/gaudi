use std::fmt::{Debug, Display, Formatter};
use clap::Parser;
use std::path::PathBuf;
use std::io::BufReader;
use std::str::FromStr;
use colored::{Color, ColoredString, Colorize, Style};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    input_file: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum VerticalDirection {
    UP,
    DOWN,
}

impl FromStr for VerticalDirection {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "up" => {
                Ok(VerticalDirection::UP)
            }
            "down" => {
                Ok(VerticalDirection::DOWN)
            }
            _ => Err(())
        }
    }
}

struct PixelData<'a> {
    ptr: &'a[u8],
}
impl <'a> PixelData<'a> {
    fn red(&self) -> u8 {
        self.ptr[0]
    }

    fn green(&self) -> u8 {
        self.ptr[1]
    }

    fn blue(&self) -> u8 {
        self.ptr[2]
    }

    fn alpha(&self) -> u8 {
        self.ptr[3]
    }

    fn is_transparent(&self) -> bool {
        self.alpha() == 0
    }

    fn as_color(&self) -> Color {
        Color::TrueColor {
            r: self.red(),
            g: self.green(),
            b: self.blue(),
        }
    }

    fn new(ptr: &'a [u8]) -> PixelData<'a> {
        PixelData {
            ptr,
        }
    }
}
impl Display for PixelData<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("rgba({}, {}, {}, {})", self.red(), self.green(), self.blue(), self.alpha()))
    }
}
impl Debug for PixelData<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
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

    println!("w {}, h {}, {} bytes", info.width, info.height, image_buffer.len());

    let mut row: usize = 0;
    loop {
        if row + 1 >= info.height as usize {
            break;
        }
        for col in 0..info.width as usize {
            let upper_pixel_idx = (row * (info.width as usize) + col) * 4;
            let upper_pixel = PixelData::new(&image_buffer[upper_pixel_idx .. upper_pixel_idx + 4]);
            let lower_pixel_idx = ((row + 1) * (info.width as usize) + col) * 4;
            let lower_pixel = PixelData::new(&image_buffer[lower_pixel_idx .. lower_pixel_idx + 4]);
            print!("{}", two_pixels_to_ascii_char(&upper_pixel, &lower_pixel));
        }
        println!();
        row += 2;
    }
}

fn two_pixels_to_ascii_char(upper_pixel: &PixelData, lower_pixel: &PixelData) -> ColoredString {
    if upper_pixel.is_transparent() && lower_pixel.is_transparent() {
        return " ".clear()
    }

    if upper_pixel.is_transparent() {
        assert!(!lower_pixel.is_transparent());
        return "▄".color(lower_pixel.as_color())
    }

    if lower_pixel.is_transparent() {
        assert!(!upper_pixel.is_transparent());
        return "▀".color(upper_pixel.as_color())
    }

    "▄".color(lower_pixel.as_color()).on_color(upper_pixel.as_color())
}