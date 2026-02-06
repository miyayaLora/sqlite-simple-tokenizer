#[cfg(feature = "build_extension")]
mod create_extension;
pub mod jieba_tokenizer;
mod load_extension;
mod utils;

use load_extension::load_fts5_extension;
use log::LevelFilter;
use rusqlite::Connection;
use rusqlite_ext::error::Error;
use utils::init_logging;

pub fn load(connection: &Connection) -> Result<(), Error> {
    load_with_loglevel(connection, LevelFilter::Info)
}

pub fn load_with_loglevel(connection: &Connection, log_level: LevelFilter) -> Result<(), Error> {
    // 设置 log
    init_logging(log_level);
    // 加载 fts5 拓展
    load_fts5_extension(connection)
}

#[cfg(test)]
mod tests {
    use crate::load;
    use rusqlite::Connection;

    #[test]
    fn test_jieba() {
        let conn = Connection::open_in_memory().unwrap();
        load(&conn).unwrap();
        // 创建一个测试表
        conn.execute(
            "CREATE VIRTUAL TABLE t1 USING fts5(text, tokenize = 'jieba');",
            [],
        )
        .unwrap();
        // 插入数据
        conn.execute(
            r#"INSERT INTO t1(text) VALUES ('中华人民共和国国歌'),('静夜思'),('国家'),('举头望明月'),('like'),('liking'),('liked'),('I''m making a sqlite tokenizer'),('I''m learning English');"#,
            [],
        )
            .unwrap();
        let mut stmt = conn
            .prepare("SELECT * FROM t1 WHERE text MATCH '国歌';")
            .unwrap();
        let result = stmt
            .query_map([], |row| Ok(row.get::<_, String>(0).unwrap()))
            .unwrap();
        let mut vec = Vec::new();
        for row in result {
            let row = row.unwrap();
            vec.push(row)
        }
        assert_eq!(["中华人民共和国国歌"], vec.as_slice());
    }
}
