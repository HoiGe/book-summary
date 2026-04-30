use chinese_number::{ChineseCountMethod, ChineseToNumber};
use std::cmp::Ordering;
use std::path::Path;

#[cfg(test)]
mod tests;

pub(crate) fn compare_paths(a: &str, b: &str) -> Ordering {
    match (sort_key(a), sort_key(b)) {
        (SortKey::Structured(a_vol, a_chap), SortKey::Structured(b_vol, b_chap)) => {
            (a_vol, a_chap).cmp(&(b_vol, b_chap)).then_with(|| a.cmp(b))
        }
        (SortKey::Structured(_, _), SortKey::Natural(_)) => Ordering::Less,
        (SortKey::Natural(_), SortKey::Structured(_, _)) => Ordering::Greater,
        (SortKey::Natural(a_key), SortKey::Natural(b_key)) => a_key.cmp_path(&b_key, a, b),
    }
}

pub(crate) fn parse_filename(filename: &str) -> Option<(i32, i32, String, String)> {
    let path = Path::new(filename);
    let stem = path.file_stem()?.to_str()?;
    let parts: Vec<&str> = stem.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let volume = parts[0].parse::<i32>().ok()?;
    let chapter = parts[1].parse::<i32>().ok()?;
    let title = parts[2..].join(".");
    Some((volume, chapter, title, filename.to_string()))
}

enum SortKey {
    Structured(i32, i32),
    Natural(NaturalKey),
}

struct NaturalKey {
    text: String,
    has_number: bool,
}

struct NumberToken {
    value: String,
    end: usize,
    is_sortable: bool,
}

impl NaturalKey {
    fn cmp_path(&self, other: &Self, original: &str, other_original: &str) -> Ordering {
        match (self.has_number, other.has_number) {
            (true, true) => alphanumeric_sort::compare_str(&self.text, &other.text)
                .then_with(|| original.cmp(other_original)),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => Ordering::Equal,
        }
    }
}

fn sort_key(filename: &str) -> SortKey {
    if let Some((vol, chap, _, _)) = parse_filename(filename) {
        SortKey::Structured(vol, chap)
    } else {
        SortKey::Natural(natural_key(filename))
    }
}

fn natural_key(filename: &str) -> NaturalKey {
    let stem = match file_stem(filename) {
        Some(stem) => stem,
        None => filename.to_string(),
    };
    let mut text = String::new();
    let mut has_number = false;
    let mut byte_index = 0;

    while byte_index < stem.len() {
        let Some(char) = stem[byte_index..].chars().next() else {
            break;
        };

        if let Some(token) = number_token(&stem, byte_index, char) {
            if token.is_sortable {
                has_number = true;
                remove_chapter_prefix_if_needed(&mut text, &stem, token.end);
            }
            text.push_str(&token.value);
            byte_index = token.end;
        } else {
            text.push(char);
            byte_index += char.len_utf8();
        }
    }

    NaturalKey { text, has_number }
}

fn number_token(text: &str, start: usize, char: char) -> Option<NumberToken> {
    if char.is_ascii_digit() {
        let (digits, end) = take_ascii_digits(text, start);
        return Some(NumberToken {
            value: digits,
            end,
            is_sortable: true,
        });
    }

    if !is_chinese_number_char(char) {
        return None;
    }

    let (candidate, end_offset) = take_chinese_candidate(&text[start..]);
    let end = start + end_offset;
    match parse_chinese_number(&candidate) {
        Some(number) if should_use_chinese_number(text, start, end) => Some(NumberToken {
            value: number.to_string(),
            end,
            is_sortable: true,
        }),
        _ => Some(NumberToken {
            value: candidate,
            end,
            is_sortable: false,
        }),
    }
}

fn take_ascii_digits(text: &str, start: usize) -> (String, usize) {
    let mut digits = String::new();
    let mut end = start;

    while end < text.len() {
        let Some(char) = text[end..].chars().next() else {
            break;
        };
        if !char.is_ascii_digit() {
            break;
        }
        digits.push(char);
        end += char.len_utf8();
    }

    (digits, end)
}

fn take_chinese_candidate(input: &str) -> (String, usize) {
    let mut candidate = String::new();
    let mut end = 0;

    for (index, char) in input.char_indices() {
        if !is_chinese_number_char(char) {
            break;
        }
        candidate.push(char);
        end = index + char.len_utf8();
    }

    (candidate, end)
}

fn remove_chapter_prefix_if_needed(sort_text: &mut String, original: &str, number_end: usize) {
    if sort_text.ends_with('第') && has_chapter_unit_after(original, number_end) {
        sort_text.pop();
    }
}

fn should_use_chinese_number(text: &str, start: usize, end: usize) -> bool {
    has_chapter_unit_after(text, end)
        || (starts_at_filename_boundary(text, start) && has_boundary_after(text, end))
}

fn starts_at_filename_boundary(text: &str, start: usize) -> bool {
    text[..start]
        .chars()
        .all(|char| char.is_whitespace() || matches!(char, '-' | '_' | '.' | '(' | '['))
}

fn has_boundary_after(text: &str, end: usize) -> bool {
    match text[end..].chars().next() {
        Some(char) => char.is_whitespace() || matches!(char, '-' | '_' | '.' | ')' | ']'),
        None => true,
    }
}

fn has_chapter_unit_after(text: &str, index: usize) -> bool {
    match text[index..].chars().next() {
        Some(char) => is_chapter_unit_char(char),
        None => false,
    }
}

fn is_chapter_unit_char(char: char) -> bool {
    CHAPTER_UNITS.contains(char)
}

fn parse_chinese_number(input: &str) -> Option<i32> {
    if input.is_empty() {
        return None;
    }

    let parsed = if input.chars().all(is_chinese_digit_char) {
        input.to_number_naive()
    } else {
        input.to_number(ChineseCountMethod::TenThousand)
    };

    parsed.ok()
}

fn is_chinese_digit_char(char: char) -> bool {
    CHINESE_DIGITS.contains(char)
}

fn is_chinese_number_char(char: char) -> bool {
    CHINESE_NUMBER_CHARS.contains(char)
}

const CHAPTER_UNITS: &str = "章节節回卷话話篇部集幕折辑輯册冊";
const CHINESE_DIGITS: &str = "零〇一壹二两贰貳弍三叁參四肆五伍六陆陸七柒八捌九玖";
const CHINESE_NUMBER_CHARS: &str =
    "零〇一壹二两贰貳弍三叁參四肆五伍六陆陸七柒八捌九玖十拾百佰千仟万萬";

fn file_stem(file_path: &str) -> Option<String> {
    Path::new(file_path)
        .file_stem()?
        .to_str()
        .map(str::to_string)
}
