//! Synthetic, seeded corpus shared by the pure regression and the opt-in PG/browser probe.
use unicode_normalization::UnicodeNormalization;
use vink_app::domain::document::DocumentData;

pub const SEED: u64 = 229;
pub const SHAPES: [&str; 7] = [
    "ascii_repeat",
    "vi_nfc",
    "vi_nfd",
    "emoji",
    "poem_tabs",
    "tiny_blocks",
    "escaping",
];

pub fn corpus(shape: &str, scalars: usize) -> DocumentData {
    let alphabet: Vec<char> = match shape {
        "ascii_repeat" => "a",
        "emoji" => "😀🙂🌧🌿🌙🖋📚🎶",
        "escaping" => "a\"\\\t<&>",
        "tiny_blocks" => "abcdefghijklmnop",
        _ => "aăâbcdđeêghiklmnoôơpqrstuưvxyáàảãạắằẳẵặấầẩẫậéèẻẽẹếềểễệíìỉĩịóòỏõọốồổỗộớờởỡợúùủũụứừửữựýỳỷỹỵ",
    }.chars().collect();
    let mut state = SEED;
    let chars: Vec<char> = (0..scalars)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            alphabet[((state >> 32) as usize) % alphabet.len()]
        })
        .collect();
    let lines: Vec<String> = if shape == "poem_tabs" {
        chars
            .chunks(64)
            .map(|chunk| {
                let mut s: String = chunk.iter().collect();
                // Indentation counts toward the same scalar budget, and a one-char tail stays valid.
                if chunk.len() > 1 {
                    s.replace_range(..s.chars().next().unwrap().len_utf8(), "\t");
                }
                s
            })
            .collect()
    } else if shape == "tiny_blocks" {
        chars.iter().map(char::to_string).collect()
    } else {
        vec![chars.iter().collect()]
    };
    let block_lines = if shape == "poem_tabs" { 4 } else { 1 };
    // Lines joined by `\n`, stanzas/paragraphs separated by one blank line; the default format
    // keeps every byte, so `vi_nfd` stays decomposed.
    let source = lines
        .chunks(block_lines)
        .map(|lines| {
            lines
                .iter()
                .map(|s| {
                    if shape == "vi_nfd" {
                        s.nfd().collect()
                    } else {
                        s.clone()
                    }
                })
                .collect::<Vec<String>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    DocumentData {
        format: "default".into(),
        source,
    }
}

/// Replace one non-line-break scalar without changing line boundaries or the content length.
pub fn edit(data: &mut DocumentData, at: usize) {
    let (start, c) = data
        .source
        .char_indices()
        .filter(|(_, c)| *c != '\n')
        .nth(at)
        .expect("edit outside corpus");
    data.source.replace_range(
        start..start + c.len_utf8(),
        if c == 'Z' { "Y" } else { "Z" },
    );
}
