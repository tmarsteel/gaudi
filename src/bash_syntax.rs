use std::fmt;
use std::fmt::{Display};
use ansi_term::ANSIGenericString;

pub fn write_with_minimal_control_sequences(spans: Vec<ANSIGenericString<'static, str>>, fmt: &mut fmt::Formatter) -> fmt::Result {
    if spans.is_empty() {
        return Ok(())
    }

    let mut previous_style = spans.first().unwrap().style_ref().clone();
    Display::fmt(&previous_style.prefix(), fmt)?;

    for escape in &spans {
        let next_style = escape.style_ref().clone();
        Display::fmt(&previous_style.infix(next_style), fmt)?;
        fmt.write_str(&escape)?;
        previous_style = next_style
    }

    Display::fmt(&(spans.last().unwrap().style_ref().suffix()), fmt)
}

pub fn escape_for_string_content(payload: &String) -> String {
    let mut out = String::with_capacity(payload.len());

    for char in payload.chars() {
        match char {
            '\\' | '"' => {
                out.push_str("\\")
            },
            '\u{1b}' => {
                out.push_str("\\e");
                continue;
            }
            '\u{0a}' => {
                out.push_str("\\n");
                continue;
            }
            '\u{1d}' => {
                out.push_str("\\r");
                continue;
            }
            _ => {
                if !char.is_ascii() || char.is_control() {
                    out.push_str(&format!("\\u{:04x}", char as u32));
                    continue;
                }
            }
        }

        out.push(char);
    }

    out
}