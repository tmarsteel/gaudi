pub mod colormath;
pub mod bash_syntax;

use std::{env};
use std::fmt::{Debug, Display, Formatter, Write};
use std::io::BufReader;
use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::str::FromStr;
use image::{DynamicImage, GenericImageView, ImageReader, Rgba,};
use ansi_term::{ANSIGenericString, Colour, Style};
use image::imageops::FilterType;
use crate::bash_syntax::escape_for_string_content;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    input_file: PathBuf,

    #[arg(long, value_enum, default_value = "up")]
    vertical_gravity: VerticalDirection,

    #[arg(long)]
    resize_to_width: Option<u32>,

    #[arg(long, value_enum, default_value = "nearest")]
    resize_filter: RequestedFilterType,

    #[arg(long, default_value = "auto")]
    color_mode: RequestedColorMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum VerticalDirection {
    UP,
    DOWN,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RequestedColorMode {
    TrueColor,
    ANSI,
    M256Color,
    AUTO,
}
impl FromStr for RequestedColorMode {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let input_lowercase = s.to_lowercase();
        match input_lowercase.as_str() {
            "truecolor" => Ok(RequestedColorMode::TrueColor),
            "ansi" => Ok(RequestedColorMode::ANSI),
            "256" => Ok(RequestedColorMode::M256Color),
            "auto" => Ok(RequestedColorMode::AUTO),
            _ => Err("Invalid color mode, use truecolor, ansi, 256 or auto"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum RequestedFilterType {
    Lanczos3,
    Nearest,
    Triangle,
    CatmullRom,
    Gaussian
}
impl Into<FilterType> for RequestedFilterType {
    fn into(self) -> FilterType {
        match self {
            RequestedFilterType::Lanczos3 => FilterType::Lanczos3,
            RequestedFilterType::Nearest => FilterType::Nearest,
            RequestedFilterType::Triangle => FilterType::Triangle,
            RequestedFilterType::CatmullRom => FilterType::CatmullRom,
            RequestedFilterType::Gaussian => FilterType::Gaussian,
        }
    }
}

fn main() {
    let args = Args::parse();

    let input_file = std::fs::File::open(&args.input_file).unwrap_or_else(|e| panic!("Could not open file {}: {}", args.input_file.display(), e));
    let mut image = DynamicImage::ImageRgba8(ImageReader::new(BufReader::new(input_file))
        .with_guessed_format().unwrap_or_else(|e| panic!("Failed to read image data from {}: {}", args.input_file.display(), e))
        .decode().unwrap_or_else(|e| panic!("Failed to decode image data from {}: {}", args.input_file.display(), e))
        .into_rgba8()
    );

    if let Some(resize_to_width) = args.resize_to_width {
        let factor = resize_to_width as f32 / image.width() as f32;
        let new_height = (image.height() as f32 * factor) as u32;
        image = image.resize(resize_to_width, new_height, args.resize_filter.into());
    }

    let explicit_mapper: Option<&dyn Fn(&Rgba<u8>) -> Colour> = match args.color_mode {
        RequestedColorMode::TrueColor => Some(&colormath::color_mapping_truecolor),
        RequestedColorMode::ANSI => Some(&colormath::color_mapping_ansi),
        RequestedColorMode::M256Color => Some(&colormath::color_mapping_256),
        RequestedColorMode::AUTO => None
    };

    let snippet = ImageEmittingBashSnippet { image, explicit_mapper };
    println!("{}", snippet);
}

fn image_to_ascii(
    image: &DynamicImage,
    vertical_gravity: VerticalDirection,
    color_mapper: &dyn Fn(&Rgba<u8>) -> Colour,
) -> Vec<ANSIGenericString<'static, str>> {
    let mut as_string: Vec<ANSIGenericString<'static, str>> = Vec::with_capacity((image.width() as usize + 1) * (image.height() as usize / 2 + 1));
    let mut row: u32 = 0;
    if image.height() % 2 != 0 && vertical_gravity == VerticalDirection::DOWN {
        for col in 0..image.width() {
            let upper_pixel = Rgba::from([0, 0, 0, 0]);
            let lower_pixel = image.get_pixel(col, 0);
            as_string.push(two_pixels_to_ascii_char(&upper_pixel, &lower_pixel, color_mapper));
        }
        as_string.push(Style::default().paint("\n"));
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
        as_string.push(Style::default().paint("\n"));
        row += 2;
    }

    if image.height() % 2 != 0 && vertical_gravity == VerticalDirection::UP {
        for col in 0..image.width() {
            let upper_pixel = image.get_pixel(col, image.height() - 1);
            let lower_pixel = Rgba::from([0, 0, 0, 0]);
            as_string.push(two_pixels_to_ascii_char(&upper_pixel, &lower_pixel, color_mapper));
        }
        as_string.push(Style::default().paint("\n"));
    }

    as_string
}

fn is_transparent(pixel: &Rgba<u8>) -> bool {
    pixel[3] == 0
}

fn two_pixels_to_ascii_char(
    upper_pixel: &Rgba<u8>,
    lower_pixel: &Rgba<u8>,
    color_mapper: &dyn Fn (&Rgba<u8>) -> Colour,
) -> ANSIGenericString<'static, str> {
    if is_transparent(upper_pixel) && is_transparent(lower_pixel) {
        return Style::default().paint(" ");
    }

    if is_transparent(upper_pixel) {
        assert!(!is_transparent(lower_pixel));
        return color_mapper(lower_pixel).paint("▄");
    }

    if is_transparent(lower_pixel) {
        assert!(!is_transparent(upper_pixel));
        return color_mapper(upper_pixel).paint("▀");
    }

    color_mapper(lower_pixel).on(color_mapper(upper_pixel)).paint("▄")
}

struct ImageEmittingBashSnippet {
    image: DynamicImage,
    explicit_mapper: Option<&'static dyn Fn(&Rgba<u8>) -> Colour>,
}
impl Display for ImageEmittingBashSnippet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(mapper) = self.explicit_mapper {
            self.emit_bash_with_color_mapper(mapper, f)
        } else {
            f.write_str("if [[ \"$COLORTERM\" == \"truecolor\" || \"$COLORTERM\" == \"24bit\" ]]; then\n    ")?;
            self.emit_bash_with_color_mapper(&colormath::color_mapping_truecolor, f)?;
            f.write_str("\nelif [[ \"$(tput colors)\" == \"256\" ]]; then \n    ")?;
            self.emit_bash_with_color_mapper(&colormath::color_mapping_256, f)?;
            f.write_str("\nelse\n    ")?;
            self.emit_bash_with_color_mapper(&colormath::color_mapping_ansi, f)?;
            f.write_str("\nfi\n")
        }
    }
}
impl ImageEmittingBashSnippet {
    fn emit_bash_with_color_mapper(&self, mapper: &dyn Fn(&Rgba<u8>) -> Colour, f: &mut Formatter) -> std::fmt::Result {
        f.write_str("echo -e -n \"")?;
        let string_content = capture_to_string(&|f| {
            bash_syntax::write_with_minimal_control_sequences(
                image_to_ascii(&self.image, VerticalDirection::UP, mapper),
                f,
            )
        });
        f.write_str(escape_for_string_content(&string_content).as_str())?;
        f.write_str("\"")
    }
}

fn capture_to_string(formats: &dyn Fn(&mut Formatter) -> std::fmt::Result) -> String {
    let displayable = Displayable {
        formats,
    };

    format!("{}", displayable)
}

struct Displayable<'a> {
    formats: &'a dyn Fn(&mut Formatter) -> std::fmt::Result,
}
impl Display for Displayable<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        (self.formats)(f)
    }
}