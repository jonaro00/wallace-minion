type Bases = (Option<u32>, Option<u32>, Option<u32>); // upper, lower, numeric
const ASCII_BASES: Bases = (Some(0x41), Some(0x61), Some(0x30));

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

pub fn to_cool_text(text: &str, font: Font) -> String {
    let bases = cool_text_bases(font);
    let mut s = String::new();
    for c in text.chars() {
        if c.is_ascii_uppercase() && bases.0.is_some() {
            s.push(
                char::from_u32((c as u32) - ASCII_BASES.0.unwrap() + bases.0.unwrap()).unwrap_or(c),
            );
        } else if c.is_ascii_lowercase() && bases.1.is_some() {
            s.push(
                char::from_u32((c as u32) - ASCII_BASES.1.unwrap() + bases.1.unwrap()).unwrap_or(c),
            );
        } else if c.is_ascii_digit() && bases.2.is_some() {
            s.push(
                char::from_u32((c as u32) - ASCII_BASES.2.unwrap() + bases.2.unwrap()).unwrap_or(c),
            );
        } else {
            s.push(c);
        }
    }
    s
}
