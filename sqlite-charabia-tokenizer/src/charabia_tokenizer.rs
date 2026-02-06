use charabia::Tokenize;
use rusqlite::Error;
use rusqlite_ext::{TokenizeReason, Tokenizer};
use sqlite_chinese_stopword::STOPWORD;
use std::ffi::CStr;
use std::ops::Range;

/// 使用 charabia 分词器
pub struct CharabiaTokenizer {
    /// 是否启用停词表, 默认启用
    enable_chinese_stopword: bool,
}

impl Default for CharabiaTokenizer {
    fn default() -> Self {
        Self {
            enable_chinese_stopword: true,
        }
    }
}

impl CharabiaTokenizer {
    /// 不启用停词表
    pub fn disable_chinese_stopword(&mut self) {
        self.enable_chinese_stopword = false;
    }
}

impl Tokenizer for CharabiaTokenizer {
    type Global = ();

    fn name() -> &'static CStr {
        c"charabia"
    }

    fn new(_global: &Self::Global, args: Vec<String>) -> Result<Self, Error> {
        let mut tokenizer = Self::default();
        for arg in args {
            if arg.as_str() == "disable_chinese_stopword" {
                tokenizer.disable_chinese_stopword();
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
        let text = String::from_utf8_lossy(text).to_string();
        let str = text.as_str();
        let tokens = str.tokenize();
        for token in tokens {
            // 停词和操作符均不需要
            if token.is_word() {
                // 启用了中文停词表，并且识别出 token 是中文，那么使用中文停词表进行过滤
                if self.enable_chinese_stopword
                    && let Some(lang) = token.language
                    && lang == charabia::Language::Zho
                    && STOPWORD.contains(&token.lemma)
                {
                    continue;
                }
                let range = token.byte_start..token.byte_end;
                (push_token)(token.lemma.as_bytes(), range, false)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_charabia_token() {
        use charabia::Tokenize;

        let orig = "Thé quick (\"brown\") fox can't jump 32.3 feet, right? Brr, it's 29.3°F!";

        // tokenize the text.
        let mut tokens = orig.tokenize();

        let token = tokens.next().unwrap();

        assert_eq!(token.lemma(), "the");

        let token = tokens.next().unwrap();

        assert_eq!(token.lemma(), " ");

        let token = tokens.next().unwrap();

        assert_eq!(token.lemma(), "quick");
        assert_eq!(token.byte_start, 5);
        assert_eq!(token.byte_end, 10);
    }

    #[test]
    fn test_charabia_token_chinese_stopword() {
        use charabia::Tokenize;

        let orig = "中国的时代";

        // tokenize the text.
        let mut tokens = orig.tokenize();

        let token = tokens.next().unwrap();

        assert_eq!(token.lemma(), "中國");

        let token = tokens.next().unwrap();

        assert_eq!(token.lemma(), "的");
        // charabia 没有内置中文常用的停词表
        assert!(!token.is_stopword());

        let token = tokens.next().unwrap();

        assert_eq!(token.lemma(), "時代");
    }
}
