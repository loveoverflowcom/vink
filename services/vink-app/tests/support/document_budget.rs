//! Synthetic, seeded corpus shared by the pure regression and the opt-in PG/browser probe.
use unicode_normalization::UnicodeNormalization;
use vink_app::domain::document::{BlockData, DocumentData, SCHEMA_VERSION};

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
    DocumentData {
        schema_version: SCHEMA_VERSION,
        blocks: lines
            .chunks(block_lines)
            .map(|lines| BlockData {
                lines: lines
                    .iter()
                    .map(|s| {
                        if shape == "vi_nfd" {
                            s.nfd().collect()
                        } else {
                            s.clone()
                        }
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Replace one scalar without changing block/line boundaries or the content length.
pub fn edit(data: &mut DocumentData, at: usize) {
    let mut remaining = at;
    for line in data.blocks.iter_mut().flat_map(|b| &mut b.lines) {
        let n = line.chars().count();
        if remaining < n {
            let (start, c) = line.char_indices().nth(remaining).unwrap();
            line.replace_range(
                start..start + c.len_utf8(),
                if c == 'Z' { "Y" } else { "Z" },
            );
            return;
        }
        remaining -= n;
    }
    panic!("edit outside corpus");
}
