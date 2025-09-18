use std::env;
use std::fmt::{Debug, Display, Formatter};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::io::BufReader;
use std::str::FromStr;
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageReader, Rgba, RgbaImage};
use ansi_term::{ANSIGenericString, Colour, Style};
use colored::Color::{Black, Blue, BrightBlack, BrightBlue, BrightCyan, BrightGreen, BrightMagenta, BrightRed, BrightWhite, BrightYellow, Cyan, Green, Magenta, Red, TrueColor, White, Yellow};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    input_file: PathBuf,

    #[arg(long, value_enum, default_value = "up")]
    vertical_gravity: VerticalDirection,

    #[arg(long)]
    resize_to_width: Option<u32>,

    #[arg(long, default_value = "auto")]
    color_mode: ColorMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum VerticalDirection {
    UP,
    DOWN,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ColorMode {
    TrueColor,
    ANSI,
    M256Color,
}
impl FromStr for ColorMode {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let input_lowercase = s.to_lowercase();
        match input_lowercase.as_str() {
            "truecolor" => Ok(ColorMode::TrueColor),
            "ansi" => Ok(ColorMode::ANSI),
            "256" => Ok(ColorMode::M256Color),
            "auto" => {
                Ok(match env::var("COLORTERM") {
                    Ok(colorterm) => match colorterm.as_str() {
                        "truecolor" | "24bit" => ColorMode::TrueColor,
                        _ => ColorMode::ANSI,
                    }
                    _ => ColorMode::ANSI
                })
            }
            _ => Err("Invalid color mode, use truecolor, ansi, 256 or auto"),
        }
    }
}
impl ColorMode {
    fn color_mapper(&self) -> fn(&Rgba<u8>) -> Colour {
        match self {
            ColorMode::TrueColor => color_mapping_truecolor,
            ColorMode::ANSI => color_mapping_ansi,
            ColorMode::M256Color => color_mapping_256,
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
        image = image.resize(resize_to_width, new_height, image::imageops::FilterType::Nearest);
    }

    let slices = image_to_ascii(image, args.vertical_gravity, args.color_mode.color_mapper());
    slices.into_iter().for_each(|s| print!("{}", s))
}

fn image_to_ascii(
    image: DynamicImage,
    vertical_gravity: VerticalDirection,
    color_mapper: fn(&Rgba<u8>) -> Colour,
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
    color_mapper: fn (&Rgba<u8>) -> Colour,
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

fn color_mapping_truecolor(pixel: &Rgba<u8>) -> Colour {
    Colour::RGB(pixel[0], pixel[1], pixel[2])
}

fn color_mapping_ansi(pixel: &Rgba<u8>) -> Colour {
    let colored_v = TrueColor {
        r: pixel[0],
        g: pixel[1],
        b: pixel[2],
    };

    let closest_colored_v = pick_closest_from(
        colored_v,
        &[
            Black,
            Red,
            Green,
            Yellow,
            Blue,
            Magenta,
            Cyan,
            White,
        ],
        into_truecolor,
    ).unwrap();

    match closest_colored_v {
        Black => Colour::Black,
        Red => Colour::Red,
        Green => Colour::Red,
        Yellow => Colour::Red,
        Blue => Colour::Red,
        Magenta => Colour::Red,
        Cyan => Colour::Red,
        White => Colour::Red,
        _ => panic!()
    }
}

static ansi_colors: [Colour; 256] = [
    Colour::Fixed(0x00),
    Colour::Fixed(0x01),
    Colour::Fixed(0x02),
    Colour::Fixed(0x03),
    Colour::Fixed(0x04),
    Colour::Fixed(0x05),
    Colour::Fixed(0x06),
    Colour::Fixed(0x07),
    Colour::Fixed(0x08),
    Colour::Fixed(0x09),
    Colour::Fixed(0x0A),
    Colour::Fixed(0x0B),
    Colour::Fixed(0x0C),
    Colour::Fixed(0x0D),
    Colour::Fixed(0x0E),
    Colour::Fixed(0x0F),
    Colour::Fixed(0x10),
    Colour::Fixed(0x11),
    Colour::Fixed(0x12),
    Colour::Fixed(0x13),
    Colour::Fixed(0x14),
    Colour::Fixed(0x15),
    Colour::Fixed(0x16),
    Colour::Fixed(0x17),
    Colour::Fixed(0x18),
    Colour::Fixed(0x19),
    Colour::Fixed(0x1A),
    Colour::Fixed(0x1B),
    Colour::Fixed(0x1C),
    Colour::Fixed(0x1D),
    Colour::Fixed(0x1E),
    Colour::Fixed(0x1F),
    Colour::Fixed(0x20),
    Colour::Fixed(0x21),
    Colour::Fixed(0x22),
    Colour::Fixed(0x23),
    Colour::Fixed(0x24),
    Colour::Fixed(0x25),
    Colour::Fixed(0x26),
    Colour::Fixed(0x27),
    Colour::Fixed(0x28),
    Colour::Fixed(0x29),
    Colour::Fixed(0x2A),
    Colour::Fixed(0x2B),
    Colour::Fixed(0x2C),
    Colour::Fixed(0x2D),
    Colour::Fixed(0x2E),
    Colour::Fixed(0x2F),
    Colour::Fixed(0x30),
    Colour::Fixed(0x31),
    Colour::Fixed(0x32),
    Colour::Fixed(0x33),
    Colour::Fixed(0x34),
    Colour::Fixed(0x35),
    Colour::Fixed(0x36),
    Colour::Fixed(0x37),
    Colour::Fixed(0x38),
    Colour::Fixed(0x39),
    Colour::Fixed(0x3A),
    Colour::Fixed(0x3B),
    Colour::Fixed(0x3C),
    Colour::Fixed(0x3D),
    Colour::Fixed(0x3E),
    Colour::Fixed(0x3F),
    Colour::Fixed(0x40),
    Colour::Fixed(0x41),
    Colour::Fixed(0x42),
    Colour::Fixed(0x43),
    Colour::Fixed(0x44),
    Colour::Fixed(0x45),
    Colour::Fixed(0x46),
    Colour::Fixed(0x47),
    Colour::Fixed(0x48),
    Colour::Fixed(0x49),
    Colour::Fixed(0x4A),
    Colour::Fixed(0x4B),
    Colour::Fixed(0x4C),
    Colour::Fixed(0x4D),
    Colour::Fixed(0x4E),
    Colour::Fixed(0x4F),
    Colour::Fixed(0x50),
    Colour::Fixed(0x51),
    Colour::Fixed(0x52),
    Colour::Fixed(0x53),
    Colour::Fixed(0x54),
    Colour::Fixed(0x55),
    Colour::Fixed(0x56),
    Colour::Fixed(0x57),
    Colour::Fixed(0x58),
    Colour::Fixed(0x59),
    Colour::Fixed(0x5A),
    Colour::Fixed(0x5B),
    Colour::Fixed(0x5C),
    Colour::Fixed(0x5D),
    Colour::Fixed(0x5E),
    Colour::Fixed(0x5F),
    Colour::Fixed(0x60),
    Colour::Fixed(0x61),
    Colour::Fixed(0x62),
    Colour::Fixed(0x63),
    Colour::Fixed(0x64),
    Colour::Fixed(0x65),
    Colour::Fixed(0x66),
    Colour::Fixed(0x67),
    Colour::Fixed(0x68),
    Colour::Fixed(0x69),
    Colour::Fixed(0x6A),
    Colour::Fixed(0x6B),
    Colour::Fixed(0x6C),
    Colour::Fixed(0x6D),
    Colour::Fixed(0x6E),
    Colour::Fixed(0x6F),
    Colour::Fixed(0x70),
    Colour::Fixed(0x71),
    Colour::Fixed(0x72),
    Colour::Fixed(0x73),
    Colour::Fixed(0x74),
    Colour::Fixed(0x75),
    Colour::Fixed(0x76),
    Colour::Fixed(0x77),
    Colour::Fixed(0x78),
    Colour::Fixed(0x79),
    Colour::Fixed(0x7A),
    Colour::Fixed(0x7B),
    Colour::Fixed(0x7C),
    Colour::Fixed(0x7D),
    Colour::Fixed(0x7E),
    Colour::Fixed(0x7F),
    Colour::Fixed(0x80),
    Colour::Fixed(0x81),
    Colour::Fixed(0x82),
    Colour::Fixed(0x83),
    Colour::Fixed(0x84),
    Colour::Fixed(0x85),
    Colour::Fixed(0x86),
    Colour::Fixed(0x87),
    Colour::Fixed(0x88),
    Colour::Fixed(0x89),
    Colour::Fixed(0x8A),
    Colour::Fixed(0x8B),
    Colour::Fixed(0x8C),
    Colour::Fixed(0x8D),
    Colour::Fixed(0x8E),
    Colour::Fixed(0x8F),
    Colour::Fixed(0x90),
    Colour::Fixed(0x91),
    Colour::Fixed(0x92),
    Colour::Fixed(0x93),
    Colour::Fixed(0x94),
    Colour::Fixed(0x95),
    Colour::Fixed(0x96),
    Colour::Fixed(0x97),
    Colour::Fixed(0x98),
    Colour::Fixed(0x99),
    Colour::Fixed(0x9A),
    Colour::Fixed(0x9B),
    Colour::Fixed(0x9C),
    Colour::Fixed(0x9D),
    Colour::Fixed(0x9E),
    Colour::Fixed(0x9F),
    Colour::Fixed(0xA0),
    Colour::Fixed(0xA1),
    Colour::Fixed(0xA2),
    Colour::Fixed(0xA3),
    Colour::Fixed(0xA4),
    Colour::Fixed(0xA5),
    Colour::Fixed(0xA6),
    Colour::Fixed(0xA7),
    Colour::Fixed(0xA8),
    Colour::Fixed(0xA9),
    Colour::Fixed(0xAA),
    Colour::Fixed(0xAB),
    Colour::Fixed(0xAC),
    Colour::Fixed(0xAD),
    Colour::Fixed(0xAE),
    Colour::Fixed(0xAF),
    Colour::Fixed(0xB0),
    Colour::Fixed(0xB1),
    Colour::Fixed(0xB2),
    Colour::Fixed(0xB3),
    Colour::Fixed(0xB4),
    Colour::Fixed(0xB5),
    Colour::Fixed(0xB6),
    Colour::Fixed(0xB7),
    Colour::Fixed(0xB8),
    Colour::Fixed(0xB9),
    Colour::Fixed(0xBA),
    Colour::Fixed(0xBB),
    Colour::Fixed(0xBC),
    Colour::Fixed(0xBD),
    Colour::Fixed(0xBE),
    Colour::Fixed(0xBF),
    Colour::Fixed(0xC0),
    Colour::Fixed(0xC1),
    Colour::Fixed(0xC2),
    Colour::Fixed(0xC3),
    Colour::Fixed(0xC4),
    Colour::Fixed(0xC5),
    Colour::Fixed(0xC6),
    Colour::Fixed(0xC7),
    Colour::Fixed(0xC8),
    Colour::Fixed(0xC9),
    Colour::Fixed(0xCA),
    Colour::Fixed(0xCB),
    Colour::Fixed(0xCC),
    Colour::Fixed(0xCD),
    Colour::Fixed(0xCE),
    Colour::Fixed(0xCF),
    Colour::Fixed(0xD0),
    Colour::Fixed(0xD1),
    Colour::Fixed(0xD2),
    Colour::Fixed(0xD3),
    Colour::Fixed(0xD4),
    Colour::Fixed(0xD5),
    Colour::Fixed(0xD6),
    Colour::Fixed(0xD7),
    Colour::Fixed(0xD8),
    Colour::Fixed(0xD9),
    Colour::Fixed(0xDA),
    Colour::Fixed(0xDB),
    Colour::Fixed(0xDC),
    Colour::Fixed(0xDD),
    Colour::Fixed(0xDE),
    Colour::Fixed(0xDF),
    Colour::Fixed(0xE0),
    Colour::Fixed(0xE1),
    Colour::Fixed(0xE2),
    Colour::Fixed(0xE3),
    Colour::Fixed(0xE4),
    Colour::Fixed(0xE5),
    Colour::Fixed(0xE6),
    Colour::Fixed(0xE7),
    Colour::Fixed(0xE8),
    Colour::Fixed(0xE9),
    Colour::Fixed(0xEA),
    Colour::Fixed(0xEB),
    Colour::Fixed(0xEC),
    Colour::Fixed(0xED),
    Colour::Fixed(0xEE),
    Colour::Fixed(0xEF),
    Colour::Fixed(0xF0),
    Colour::Fixed(0xF1),
    Colour::Fixed(0xF2),
    Colour::Fixed(0xF3),
    Colour::Fixed(0xF4),
    Colour::Fixed(0xF5),
    Colour::Fixed(0xF6),
    Colour::Fixed(0xF7),
    Colour::Fixed(0xF8),
    Colour::Fixed(0xF9),
    Colour::Fixed(0xFA),
    Colour::Fixed(0xFB),
    Colour::Fixed(0xFC),
    Colour::Fixed(0xFD),
    Colour::Fixed(0xFE),
    Colour::Fixed(0xFF),
];

static ansi_color_to_truecolor: [(u8, u8, u8); 256] = [
    (0x00, 0x00, 0x00),
    (0x80, 0x00, 0x00),
    (0x00, 0x80, 0x00),
    (0x80, 0x80, 0x00),
    (0x00, 0x00, 0x80),
    (0x80, 0x00, 0x80),
    (0x00, 0x80, 0x80),
    (0xc0, 0xc0, 0xc0),
    (0x80, 0x80, 0x80),
    (0xff, 0x00, 0x00),
    (0x00, 0xff, 0x00),
    (0xff, 0xff, 0x00),
    (0x00, 0x00, 0xff),
    (0xff, 0x00, 0xff),
    (0x00, 0xff, 0xff),
    (0xff, 0xff, 0xff),
    (0x00, 0x00, 0x00),
    (0x00, 0x00, 0x5f),
    (0x00, 0x00, 0x87),
    (0x00, 0x00, 0xaf),
    (0x00, 0x00, 0xd7),
    (0x00, 0x00, 0xff),
    (0x00, 0x5f, 0x00),
    (0x00, 0x5f, 0x5f),
    (0x00, 0x5f, 0x87),
    (0x00, 0x5f, 0xaf),
    (0x00, 0x5f, 0xd7),
    (0x00, 0x5f, 0xff),
    (0x00, 0x87, 0x00),
    (0x00, 0x87, 0x5f),
    (0x00, 0x87, 0x87),
    (0x00, 0x87, 0xaf),
    (0x00, 0x87, 0xd7),
    (0x00, 0x87, 0xff),
    (0x00, 0xaf, 0x00),
    (0x00, 0xaf, 0x5f),
    (0x00, 0xaf, 0x87),
    (0x00, 0xaf, 0xaf),
    (0x00, 0xaf, 0xd7),
    (0x00, 0xaf, 0xff),
    (0x00, 0xd7, 0x00),
    (0x00, 0xd7, 0x5f),
    (0x00, 0xd7, 0x87),
    (0x00, 0xd7, 0xaf),
    (0x00, 0xd7, 0xd7),
    (0x00, 0xd7, 0xff),
    (0x00, 0xff, 0x00),
    (0x00, 0xff, 0x5f),
    (0x00, 0xff, 0x87),
    (0x00, 0xff, 0xaf),
    (0x00, 0xff, 0xd7),
    (0x00, 0xff, 0xff),
    (0x5f, 0x00, 0x00),
    (0x5f, 0x00, 0x5f),
    (0x5f, 0x00, 0x87),
    (0x5f, 0x00, 0xaf),
    (0x5f, 0x00, 0xd7),
    (0x5f, 0x00, 0xff),
    (0x5f, 0x5f, 0x00),
    (0x5f, 0x5f, 0x5f),
    (0x5f, 0x5f, 0x87),
    (0x5f, 0x5f, 0xaf),
    (0x5f, 0x5f, 0xd7),
    (0x5f, 0x5f, 0xff),
    (0x5f, 0x87, 0x00),
    (0x5f, 0x87, 0x5f),
    (0x5f, 0x87, 0x87),
    (0x5f, 0x87, 0xaf),
    (0x5f, 0x87, 0xd7),
    (0x5f, 0x87, 0xff),
    (0x5f, 0xaf, 0x00),
    (0x5f, 0xaf, 0x5f),
    (0x5f, 0xaf, 0x87),
    (0x5f, 0xaf, 0xaf),
    (0x5f, 0xaf, 0xd7),
    (0x5f, 0xaf, 0xff),
    (0x5f, 0xd7, 0x00),
    (0x5f, 0xd7, 0x5f),
    (0x5f, 0xd7, 0x87),
    (0x5f, 0xd7, 0xaf),
    (0x5f, 0xd7, 0xd7),
    (0x5f, 0xd7, 0xff),
    (0x5f, 0xff, 0x00),
    (0x5f, 0xff, 0x5f),
    (0x5f, 0xff, 0x87),
    (0x5f, 0xff, 0xaf),
    (0x5f, 0xff, 0xd7),
    (0x5f, 0xff, 0xff),
    (0x87, 0x00, 0x00),
    (0x87, 0x00, 0x5f),
    (0x87, 0x00, 0x87),
    (0x87, 0x00, 0xaf),
    (0x87, 0x00, 0xd7),
    (0x87, 0x00, 0xff),
    (0x87, 0x5f, 0x00),
    (0x87, 0x5f, 0x5f),
    (0x87, 0x5f, 0x87),
    (0x87, 0x5f, 0xaf),
    (0x87, 0x5f, 0xd7),
    (0x87, 0x5f, 0xff),
    (0x87, 0x87, 0x00),
    (0x87, 0x87, 0x5f),
    (0x87, 0x87, 0x87),
    (0x87, 0x87, 0xaf),
    (0x87, 0x87, 0xd7),
    (0x87, 0x87, 0xff),
    (0x87, 0xaf, 0x00),
    (0x87, 0xaf, 0x5f),
    (0x87, 0xaf, 0x87),
    (0x87, 0xaf, 0xaf),
    (0x87, 0xaf, 0xd7),
    (0x87, 0xaf, 0xff),
    (0x87, 0xd7, 0x00),
    (0x87, 0xd7, 0x5f),
    (0x87, 0xd7, 0x87),
    (0x87, 0xd7, 0xaf),
    (0x87, 0xd7, 0xd7),
    (0x87, 0xd7, 0xff),
    (0x87, 0xff, 0x00),
    (0x87, 0xff, 0x5f),
    (0x87, 0xff, 0x87),
    (0x87, 0xff, 0xaf),
    (0x87, 0xff, 0xd7),
    (0x87, 0xff, 0xff),
    (0xaf, 0x00, 0x00),
    (0xaf, 0x00, 0x5f),
    (0xaf, 0x00, 0x87),
    (0xaf, 0x00, 0xaf),
    (0xaf, 0x00, 0xd7),
    (0xaf, 0x00, 0xff),
    (0xaf, 0x5f, 0x00),
    (0xaf, 0x5f, 0x5f),
    (0xaf, 0x5f, 0x87),
    (0xaf, 0x5f, 0xaf),
    (0xaf, 0x5f, 0xd7),
    (0xaf, 0x5f, 0xff),
    (0xaf, 0x87, 0x00),
    (0xaf, 0x87, 0x5f),
    (0xaf, 0x87, 0x87),
    (0xaf, 0x87, 0xaf),
    (0xaf, 0x87, 0xd7),
    (0xaf, 0x87, 0xff),
    (0xaf, 0xaf, 0x00),
    (0xaf, 0xaf, 0x5f),
    (0xaf, 0xaf, 0x87),
    (0xaf, 0xaf, 0xaf),
    (0xaf, 0xaf, 0xd7),
    (0xaf, 0xaf, 0xff),
    (0xaf, 0xd7, 0x00),
    (0xaf, 0xd7, 0x5f),
    (0xaf, 0xd7, 0x87),
    (0xaf, 0xd7, 0xaf),
    (0xaf, 0xd7, 0xd7),
    (0xaf, 0xd7, 0xff),
    (0xaf, 0xff, 0x00),
    (0xaf, 0xff, 0x5f),
    (0xaf, 0xff, 0x87),
    (0xaf, 0xff, 0xaf),
    (0xaf, 0xff, 0xd7),
    (0xaf, 0xff, 0xff),
    (0xd7, 0x00, 0x00),
    (0xd7, 0x00, 0x5f),
    (0xd7, 0x00, 0x87),
    (0xd7, 0x00, 0xaf),
    (0xd7, 0x00, 0xd7),
    (0xd7, 0x00, 0xff),
    (0xd7, 0x5f, 0x00),
    (0xd7, 0x5f, 0x5f),
    (0xd7, 0x5f, 0x87),
    (0xd7, 0x5f, 0xaf),
    (0xd7, 0x5f, 0xd7),
    (0xd7, 0x5f, 0xff),
    (0xd7, 0x87, 0x00),
    (0xd7, 0x87, 0x5f),
    (0xd7, 0x87, 0x87),
    (0xd7, 0x87, 0xaf),
    (0xd7, 0x87, 0xd7),
    (0xd7, 0x87, 0xff),
    (0xd7, 0xaf, 0x00),
    (0xd7, 0xaf, 0x5f),
    (0xd7, 0xaf, 0x87),
    (0xd7, 0xaf, 0xaf),
    (0xd7, 0xaf, 0xd7),
    (0xd7, 0xaf, 0xff),
    (0xd7, 0xd7, 0x00),
    (0xd7, 0xd7, 0x5f),
    (0xd7, 0xd7, 0x87),
    (0xd7, 0xd7, 0xaf),
    (0xd7, 0xd7, 0xd7),
    (0xd7, 0xd7, 0xff),
    (0xd7, 0xff, 0x00),
    (0xd7, 0xff, 0x5f),
    (0xd7, 0xff, 0x87),
    (0xd7, 0xff, 0xaf),
    (0xd7, 0xff, 0xd7),
    (0xd7, 0xff, 0xff),
    (0xff, 0x00, 0x00),
    (0xff, 0x00, 0x5f),
    (0xff, 0x00, 0x87),
    (0xff, 0x00, 0xaf),
    (0xff, 0x00, 0xd7),
    (0xff, 0x00, 0xff),
    (0xff, 0x5f, 0x00),
    (0xff, 0x5f, 0x5f),
    (0xff, 0x5f, 0x87),
    (0xff, 0x5f, 0xaf),
    (0xff, 0x5f, 0xd7),
    (0xff, 0x5f, 0xff),
    (0xff, 0x87, 0x00),
    (0xff, 0x87, 0x5f),
    (0xff, 0x87, 0x87),
    (0xff, 0x87, 0xaf),
    (0xff, 0x87, 0xd7),
    (0xff, 0x87, 0xff),
    (0xff, 0xaf, 0x00),
    (0xff, 0xaf, 0x5f),
    (0xff, 0xaf, 0x87),
    (0xff, 0xaf, 0xaf),
    (0xff, 0xaf, 0xd7),
    (0xff, 0xaf, 0xff),
    (0xff, 0xd7, 0x00),
    (0xff, 0xd7, 0x5f),
    (0xff, 0xd7, 0x87),
    (0xff, 0xd7, 0xaf),
    (0xff, 0xd7, 0xd7),
    (0xff, 0xd7, 0xff),
    (0xff, 0xff, 0x00),
    (0xff, 0xff, 0x5f),
    (0xff, 0xff, 0x87),
    (0xff, 0xff, 0xaf),
    (0xff, 0xff, 0xd7),
    (0xff, 0xff, 0xff),
    (0x08, 0x08, 0x08),
    (0x12, 0x12, 0x12),
    (0x1c, 0x1c, 0x1c),
    (0x26, 0x26, 0x26),
    (0x30, 0x30, 0x30),
    (0x3a, 0x3a, 0x3a),
    (0x44, 0x44, 0x44),
    (0x4e, 0x4e, 0x4e),
    (0x58, 0x58, 0x58),
    (0x62, 0x62, 0x62),
    (0x6c, 0x6c, 0x6c),
    (0x76, 0x76, 0x76),
    (0x80, 0x80, 0x80),
    (0x8a, 0x8a, 0x8a),
    (0x94, 0x94, 0x94),
    (0x9e, 0x9e, 0x9e),
    (0xa8, 0xa8, 0xa8),
    (0xb2, 0xb2, 0xb2),
    (0xbc, 0xbc, 0xbc),
    (0xc6, 0xc6, 0xc6),
    (0xd0, 0xd0, 0xd0),
    (0xda, 0xda, 0xda),
    (0xe4, 0xe4, 0xe4),
    (0xee, 0xee, 0xee),
];

fn color_mapping_256(pixel: &Rgba<u8>) -> Colour {
    let colored_v = TrueColor {
        r: pixel[0],
        g: pixel[1],
        b: pixel[2],
    };

    *pick_closest_from(colored_v, &ansi_colors, |c| {
        if let Colour::Fixed(index) = *c {
            ansi_color_to_truecolor[index as usize]
        } else {
            panic!("Not a fixed color");
        }
    }).unwrap()
}

fn pick_closest_from<C : Sized>(color: colored::Color, choices: &[C], candidate_to_truecolor: fn(&C) -> (u8, u8, u8)) -> Option<&C> {
    let input_as_truecolor = into_truecolor(&color);
    choices.iter()
        .min_by_key(|candidate| {
            let candidate_as_truecolor = candidate_to_truecolor(*candidate);
            euclidian_distance((input_as_truecolor.0, input_as_truecolor.1, input_as_truecolor.2), candidate_as_truecolor)
        })
}

fn euclidian_distance(ca: (u8, u8, u8), cb: (u8, u8, u8)) -> u32 {
    use std::cmp;
    let (r, g, b) = ca;
    let (r1, g1, b1) = cb;

    let rd = cmp::max(r, r1) - cmp::min(r, r1);
    let gd = cmp::max(g, g1) - cmp::min(g, g1);
    let bd = cmp::max(b, b1) - cmp::min(b, b1);
    let rd: u32 = rd.into();
    let gd: u32 = gd.into();
    let bd: u32 = bd.into();
    let distance = rd.pow(2) + gd.pow(2) + bd.pow(2);

    distance
}

fn into_truecolor(color: &colored::Color) -> (u8, u8, u8) {
    use colored::Color::*;
    match color {
        Black => (0, 0, 0),
        Red => (205, 0, 0),
        Green => (0, 205, 0),
        Yellow => (205, 205, 0),
        Blue => (0, 0, 238),
        Magenta => (205, 0, 205),
        Cyan => (0, 205, 205),
        White => (229, 229, 229),
        BrightBlack => (127, 127, 127),
        BrightRed => (255, 0, 0),
        BrightGreen => (0, 255, 0),
        BrightYellow => (255, 255, 0),
        BrightBlue => (92, 92, 255),
        BrightMagenta => (255, 0, 255),
        BrightCyan => (0, 255, 255),
        BrightWhite => (255, 255, 255),
        TrueColor { r, g, b } => (r.clone(), g.clone(), b.clone()),
    }
}