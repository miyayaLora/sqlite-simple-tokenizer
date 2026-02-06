use crate::charabia_tokenizer::CharabiaTokenizer;
use rusqlite::Connection;
use rusqlite_ext::register_tokenizer;

pub fn load_fts5_extension(connection: &Connection) -> Result<(), crate::Error> {
    // 注册 charabia_tokenizer
    register_tokenizer::<CharabiaTokenizer>(connection, ())?;
    Ok(())
}
