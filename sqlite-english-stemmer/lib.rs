use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;
use waken_snowball::{Algorithm, Stemmer};

/// 适用于英语的词干提取器
pub static EN_STEMMER: LazyLock<Stemmer> = LazyLock::new(|| Algorithm::English.stemmer());

/// 判断是不是由空字符、控制字符、ascii标点字符组成的字符串
pub fn is_space_or_ascii_punctuation_str(word: &str) -> bool {
    let mut is_space = true;
    for ch in word.chars() {
        if !ch.is_whitespace() && !ch.is_control() && !ch.is_ascii_punctuation() {
            is_space = false;
            break;
        }
    }
    is_space
}

/// 对单词做归一化，并转换成小写
///
/// 如果全部都是由 ascii 字符组成的单词，并且长度超过 1，需要返回一个变量用来提示后续步骤做词干提取
pub fn make_lowercase(word: &str, buf: &mut String) -> bool {
    buf.clear();
    let mut need_stem = true;
    for ch in word.nfkc() {
        if is_diacritic(ch) {
            continue;
        }
        if ch.is_ascii() {
            buf.push(ch.to_ascii_lowercase());
        } else {
            need_stem = false;
            buf.extend(ch.to_lowercase());
        }
    }
    if buf.len() <= 1 {
        // 单个字符不需要提取词干
        need_stem = false;
    }
    need_stem
}

fn is_diacritic(ch: char) -> bool {
    ('\u{0300}'..='\u{036f}').contains(&ch)
}
