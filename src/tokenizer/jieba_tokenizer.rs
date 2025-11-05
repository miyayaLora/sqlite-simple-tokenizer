use crate::STOPWORD;
use crate::tokenizer::{
    TokenizeReason, Tokenizer,
    utils::{EN_STEMMER, is_space_or_ascii_punctuation_str, make_lowercase},
};
use jieba_rs::Jieba;
use rusqlite::Error;
use std::ffi::CStr;
use std::ops::Range;
use std::sync::LazyLock;

static JIEBA: LazyLock<Jieba> = LazyLock::new(Jieba::new);

/// 使用 jieba 分词器
pub struct JiebaTokenizer {
    /// 是否启用停词表, 默认启用
    enable_stopword: bool,
}

impl Default for JiebaTokenizer {
    fn default() -> Self {
        Self {
            enable_stopword: true,
        }
    }
}

impl JiebaTokenizer {
    /// 不启用停词表
    pub fn disable_stopword(&mut self) {
        self.enable_stopword = false;
    }
}

impl Tokenizer for JiebaTokenizer {
    type Global = ();

    fn name() -> &'static CStr {
        c"jieba"
    }

    fn new(_global: &Self::Global, args: Vec<String>) -> Result<Self, Error> {
        let mut tokenizer = Self::default();
        for arg in args {
            if arg.as_str() == "disable_stopword" {
                tokenizer.disable_stopword();
            }
        }
        Ok(tokenizer)
    }

    fn tokenize<TKF>(
        &mut self,
        _reason: TokenizeReason,
        text: &[u8],
        mut push_token: TKF,
    ) -> Result<(), Error>
    where
        TKF: FnMut(&[u8], Range<usize>, bool) -> Result<(), Error>,
    {
        let text = String::from_utf8_lossy(text);
        // 使用 jieba 进行分词
        let mut word_buf = String::new();
        let mut index = 0_usize;
        for word in JIEBA.cut(text.as_ref(), true) {
            // sqlite 要求的是 byte 偏移量
            let range = index..index + word.len();
            index += word.len();
            // 如果是空字符、控制字符、ascii标点字符组成组成的字符串，也不处理
            if is_space_or_ascii_punctuation_str(word) {
                continue;
            }
            // 对单词做归一化处理，并且将单词转换成小写
            let need_stem = make_lowercase(word, &mut word_buf);
            if self.enable_stopword && STOPWORD.contains(word_buf.as_str()) {
                // 不处理停词
                continue;
            }
            if need_stem {
                let stemmed = EN_STEMMER.stem(word_buf.as_str()).into_owned();
                (push_token)(stemmed.as_bytes(), range, false)?;
            } else {
                (push_token)(word_buf.as_bytes(), range, false)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::JIEBA;

    #[test]
    fn test_tokenize_by_jieba_cut() {
        let text = "The quick (\"brown\") fox can't jump 32.3 feet, right? 我将点燃星海！天上的stars全部都是 eye，不要凝视";
        let words = JIEBA.cut(text, false);
        let vec = [
            "The",
            " ",
            "quick",
            " ",
            "(",
            "\"",
            "brown",
            "\"",
            ")",
            " ",
            "fox",
            " ",
            "can",
            "'",
            "t",
            " ",
            "jump",
            " ",
            "32",
            ".",
            "3",
            " ",
            "feet",
            ",",
            " ",
            "right",
            "?",
            " ",
            "我",
            "将",
            "点燃",
            "星海",
            "！",
            "天上",
            "的",
            "stars",
            "全部都是",
            " ",
            "eye",
            "，",
            "不要",
            "凝视",
        ];
        assert_eq!(words, vec);
        let words = JIEBA.cut(text, true);
        let vec = [
            "The",
            " ",
            "quick",
            " ",
            "(",
            "\"",
            "brown",
            "\"",
            ")",
            " ",
            "fox",
            " ",
            "can",
            "'",
            "t",
            " ",
            "jump",
            " ",
            "32.3",
            " ",
            "feet",
            ",",
            " ",
            "right",
            "?",
            " ",
            "我",
            "将",
            "点燃",
            "星海",
            "！",
            "天上",
            "的",
            "stars",
            "全部都是",
            " ",
            "eye",
            "，",
            "不要",
            "凝视",
        ];
        assert_eq!(words, vec);
    }

    #[test]
    fn test_tokenize_by_jieba_cut2() {
        let text = "社会主义国家";
        let words = JIEBA.cut(text, true);
        let vec = ["社会主义", "国家"];
        assert_eq!(words, vec);
    }
}
