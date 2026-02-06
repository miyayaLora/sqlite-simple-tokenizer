# sqlite-simple-tokenizer

![Crates.io License](https://img.shields.io/crates/l/sqlite-simple-tokenizer)

> 这是一个使用 `rusqlite` 构建 SQLite fts5 插件的项目，其主要功能是为 SQLite 提供中文分词。这个项目可以作为 Rust 的 crate 使用，也可以将其编译成动态库在 SQLite 中加载和使用。

## 简介

这个项目提供一种 SQLite 分词器 `jieba_tokenizer`，可处理汉语和英语两种语言，内置了汉语和英语常见停词表。汉语可以通过词典(`jieba_tokenizer`)进行分词，而英语单词在分词后，会根据 `Snowball Stemmer` 进行了词根提取。

- `jieba_tokenizer` 对于汉语的处理，是根据 `jieba.rs` 这个库进行词典分词。该分词器的分词处理，在文档查询和文档写入的时候均生效，使用 `match` 语法进行查询。

## 支持的 Rust 最小版本

这个库在维护期间，支持的 Rust 最小版本均为当前稳定版本。这个 crate 会积极采用 `Rust` 中新稳定的一些语法和标准库接口。

## 支持的 SQLite 版本

这个库基于 `rusqlite 0.38.0` 上构建，目前支持的 SQLite 版本为 `3.51.4`。较低版本的 SQLite 将无法加载此拓展。如果作为 Rust crate 使用，推荐开启 `rusqlite` 的 `bundled` 功能，使用 `rusqlite` 内置的 SQLite，减小版本不匹配而出问题的可能性。

**我们的 Semver 策略跟随 rusqlite 的 Semver。当 rusqlite 的主版本号和次版本号发生变更，我们也会跟随变更，并且发布到 crates.io。**

## 将这个库构建为动态库

- 安装 Rust 工具链

- 使用 `cargo` 进行构建

  ```shell
  cargo build --release --features build_extension
  ```

- 在 `sqlite` 中使用 `.load libsqlite_simple_tokenizer` 进行加载

## Tokenizer 基本配置和 `simple_query` 示例

```sqlite
-- 使用默认配置注册 tokenizer，jieba 默认启用停词表
CREATE VIRTUAL TABLE t1 USING fts5
(
    text,
    tokenize = 'jieba'
);

-- 不启用停词表
CREATE VIRTUAL TABLE t1 USING fts5
(
    text,
    tokenize = 'jieba disable_stopword'
);

-- 使用 simple_query 查询
SELECT *
FROM t1
WHERE text MATCH '国';
```

## 在 Rust 使用这个库

在 Rust 中使用这个分词器，需要引入 `rusqlite` 依赖， 使用 `cargo add rusqlite sqlite-simple-tokenizer` 安装依赖

```rust,not-run
let conn = Connection::open_in_memory().unwrap();
load(&conn).unwrap();
// 创建一个测试表
conn.execute(
    "CREATE VIRTUAL TABLE t1 USING fts5(text, tokenize = 'jieba');",
    [],
).unwrap();
// 插入数据
conn.execute(
    r#"INSERT INTO t1(text) VALUES ('中华人民共和国国歌'),('静夜思'),('国家'),('举头望明月'),('like'),('liking'),('liked'),('I''m making a sqlite tokenizer'),('I''m learning English');"#,
    [],
).unwrap();
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
```

## 许可

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

### 贡献

除非您另有明确说明，否则任何您提交的代码许可应按上述 Apache 和 MIT 双重许可，并没有任何附加条款或条件。