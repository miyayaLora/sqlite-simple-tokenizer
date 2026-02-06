use crate::jieba_tokenizer::JiebaTokenizer;
use rusqlite::Connection;
use rusqlite_ext::register_tokenizer;

pub fn load_fts5_extension(connection: &Connection) -> Result<(), crate::Error> {
    // 注册 jieba_tokenizer
    register_tokenizer::<JiebaTokenizer>(connection, ())?;
    Ok(())
}
