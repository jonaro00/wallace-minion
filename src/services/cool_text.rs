type Bases = (Option<u32>, Option<u32>, Option<u32>); // upper, lower, numeric
const ASCII_BASE_UPPER: u32 = 0x41;
const ASCII_BASE_LOWER: u32 = 0x61;
const ASCII_BASE_NUMERIC: u32 = 0x30;

pub enum Font {
    BoldFraktur,
    Bold,
    BoldItalic,
    BoldScript,
    Monospace,
}

fn cool_text_bases(font: Font) -> Bases {
    match font {
        Font::BoldFraktur => (Some(0x1D56C), Some(0x1D586), None),
        Font::Bold => (Some(0x1D400), Some(0x1D41A), Some(0x1D7CE)),
        Font::BoldItalic => (Some(0x1D468), Some(0x1D482), None),
        Font::BoldScript => (Some(0x1D4D0), Some(0x1D4EA), None),
        Font::Monospace => (Some(0x1D670), Some(0x1D68A), Some(0x1D7F6)),
    }
}

#[allow(clippy::unnecessary_unwrap)]
pub fn to_cool_text(text: &str, font: Font) -> String {
    let (upper, lower, numeric) = cool_text_bases(font);
    text.chars()
        .map(|c| {
            if c.is_ascii_uppercase() && upper.is_some() {
                char::from_u32((c as u32) - ASCII_BASE_UPPER + upper.unwrap())
            } else if c.is_ascii_lowercase() && lower.is_some() {
                char::from_u32((c as u32) - ASCII_BASE_LOWER + lower.unwrap())
            } else if c.is_ascii_digit() && numeric.is_some() {
                char::from_u32((c as u32) - ASCII_BASE_NUMERIC + numeric.unwrap())
            } else {
                Some(c)
            }
            .unwrap_or(c)
        })
        .collect()
}
