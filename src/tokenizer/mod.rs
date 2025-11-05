// 代码来自 https://gist.github.com/ColonelThirtyTwo/3dd1fe04e4cff0502fa70d12f3a6e72e/revisions
// 针对 Rust 和 rusqlite 的新版本做了一些调整

pub mod jieba_tokenizer;
pub mod simple_tokenizer;
mod utils;

use rusqlite::Connection;
use rusqlite::ffi::{
    FTS5_TOKEN_COLOCATED, FTS5_TOKENIZE_AUX, FTS5_TOKENIZE_DOCUMENT, FTS5_TOKENIZE_PREFIX,
    FTS5_TOKENIZE_QUERY, Fts5Tokenizer, SQLITE_ERROR, SQLITE_OK, SQLITE_PREPARE_PERSISTENT,
    fts5_api, fts5_tokenizer_v2, sqlite3_bind_pointer, sqlite3_finalize, sqlite3_prepare_v3,
    sqlite3_step, sqlite3_stmt,
};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::fmt::Formatter;
use std::ops::Range;
use std::panic::AssertUnwindSafe;

/// fts5_api 的版本，要求最低版本不能低于 3
const FTS5_API_VERSION: c_int = 3;
/// 设置 fts5_tokenizer 的版本，设置为 2，使用 v2 接口
const FTS5_TOKENIZER_VERSION: c_int = 2;

/// FTS5 请求对所提供的文本进行标记化的原因
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenizeReason {
    /// 往 FTS 表中插入或者删除文档
    Document,
    ///  对 FTS 索引执行 MATCH 查询
    Query {
        /// 查询的字符串后是否带上 “*"，
        prefix: bool,
    },
    /// 手动调用 `fts5_api.xTokenize`.
    Aux,
}

#[derive(Debug)]
pub enum IntoTokenizeReasonError {
    UnrecognizedValue(c_int),
}

impl std::fmt::Display for IntoTokenizeReasonError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnrecognizedValue(flag) => {
                write!(f, "Unrecognized flags passed to xTokenize: {flag}")
            }
        }
    }
}

impl std::error::Error for IntoTokenizeReasonError {}

impl TryFrom<c_int> for TokenizeReason {
    type Error = IntoTokenizeReasonError;

    fn try_from(value: c_int) -> Result<Self, Self::Error> {
        /// 这个值是针对 FTS 索引执行 MATCH 查询时，在查询字符串后带上 * 的特殊值
        const FTS5_TOKENIZE_QUERY_PREFIX: c_int = FTS5_TOKENIZE_QUERY | FTS5_TOKENIZE_PREFIX;
        match value {
            FTS5_TOKENIZE_DOCUMENT => Ok(Self::Document),
            FTS5_TOKENIZE_QUERY => Ok(Self::Query { prefix: false }),
            FTS5_TOKENIZE_QUERY_PREFIX => Ok(Self::Query { prefix: true }),
            FTS5_TOKENIZE_AUX => Ok(Self::Aux),
            _ => Err(IntoTokenizeReasonError::UnrecognizedValue(value)),
        }
    }
}

/// Tokenizer
pub trait Tokenizer: Sized + Send + 'static {
    /// 一个全局数据的类型
    type Global: Send + 'static;
    /// 提供一个 tokenizer 名称
    fn name() -> &'static CStr;
    /// 创建 Tokenizer 方法
    ///
    /// 在创建 Tokenizer 实例后，通过指定的全局数据访问这个实例
    ///
    /// 在 xCreate 中被调用，xCreate 的 azArg 参数转换成 Vec<String>，并以此提供给 new方法使用
    fn new(global: &Self::Global, args: Vec<String>) -> Result<Self, rusqlite::Error>;
    /// 分词的具体实现
    ///
    /// 应该检查 `text` 对象，并且对每个 `token` 调用 `push_token` 这个回调方法
    ///
    /// `push_token` 的参数有
    /// * &[u8] - token
    /// * Range<usize> - token 在文本中位置
    /// * bool - 对应 `FTS5_TOKEN_COLOCATED`
    ///
    fn tokenize<TKF>(
        &mut self,
        reason: TokenizeReason,
        text: &[u8],
        push_token: TKF,
    ) -> Result<(), rusqlite::Error>
    where
        TKF: FnMut(&[u8], Range<usize>, bool) -> Result<(), rusqlite::Error>;
}

unsafe extern "C" fn x_create<T: Tokenizer>(
    global: *mut c_void,
    args: *mut *const c_char,
    nargs: c_int,
    out_tokenizer: *mut *mut Fts5Tokenizer,
) -> c_int {
    let global = unsafe { &*global.cast::<T::Global>() };
    let args = (0..nargs as usize)
        .map(|i| unsafe { *args.add(i) })
        .map(|s| unsafe { CStr::from_ptr(s).to_string_lossy().into_owned() })
        .collect::<Vec<String>>();
    let res = std::panic::catch_unwind(AssertUnwindSafe(move || T::new(global, args)));
    match res {
        Ok(Ok(v)) => {
            let bp = Box::into_raw(Box::new(v));
            unsafe {
                *out_tokenizer = bp.cast::<Fts5Tokenizer>();
            }
            SQLITE_OK
        }
        Ok(Err(rusqlite::Error::SqliteFailure(e, _))) => e.extended_code,
        Ok(Err(_)) => SQLITE_ERROR,
        Err(msg) => {
            log::error!(
                "<{} as Tokenizer>::new panic: {}",
                std::any::type_name::<T>(),
                panic_err_to_str(&msg)
            );
            SQLITE_ERROR
        }
    }
}

unsafe extern "C" fn x_delete<T: Tokenizer>(v: *mut Fts5Tokenizer) {
    let tokenizer = unsafe { Box::from_raw(v.cast::<T>()) };
    match std::panic::catch_unwind(AssertUnwindSafe(move || drop(tokenizer))) {
        Ok(()) => {}
        Err(e) => {
            log::error!(
                "{}::drop panic: {}",
                std::any::type_name::<T>(),
                panic_err_to_str(&e)
            );
        }
    }
}

unsafe extern "C" fn x_destroy<T: Tokenizer>(v: *mut c_void) {
    let tokenizer = unsafe { Box::from_raw(v.cast::<T::Global>()) };
    match std::panic::catch_unwind(AssertUnwindSafe(move || drop(tokenizer))) {
        Ok(()) => {}
        Err(e) => {
            log::error!(
                "{}::drop panic: {}",
                std::any::type_name::<T::Global>(),
                panic_err_to_str(&e)
            );
        }
    }
}

/// 忽略 locale 配置
unsafe extern "C" fn x_tokenize<T: Tokenizer>(
    this: *mut Fts5Tokenizer,
    ctx: *mut c_void,
    flag: c_int,
    data: *const c_char,
    data_len: c_int,
    _locale: *const c_char,
    _locale_len: c_int,
    push_token: Option<
        unsafe extern "C" fn(*mut c_void, c_int, *const c_char, c_int, c_int, c_int) -> c_int,
    >,
) -> c_int {
    let this = unsafe { &mut *this.cast::<T>() };
    let reason = match TokenizeReason::try_from(flag) {
        Ok(reason) => reason,
        Err(error) => {
            log::error!("{error}");
            return SQLITE_ERROR;
        }
    };

    let data = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), data_len as usize) };

    let push_token = push_token.expect("No provide push token function");
    let push_token = |token: &[u8],
                      Range { start, end }: Range<usize>,
                      colocated: bool|
     -> Result<(), rusqlite::Error> {
        let token_len: c_int = token.len().try_into().expect("Token is too long");
        assert!(
            start <= data.len() && end <= data.len(),
            "Token range is invalid. Range is [{start}..{end}], data length is {}",
            data.len(),
        );
        let flags = if colocated { FTS5_TOKEN_COLOCATED } else { 0 };

        let res = unsafe {
            (push_token)(
                ctx,
                flags,
                token.as_ptr().cast::<c_char>(),
                token_len,
                start as c_int,
                end as c_int,
            )
        };
        if res == SQLITE_OK {
            Ok(())
        } else {
            Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(res),
                None,
            ))
        }
    };

    match std::panic::catch_unwind(AssertUnwindSafe(|| this.tokenize(reason, data, push_token))) {
        Ok(Ok(())) => SQLITE_OK,
        Ok(Err(rusqlite::Error::SqliteFailure(e, _))) => e.extended_code,
        Ok(Err(_)) => SQLITE_ERROR,
        Err(msg) => {
            log::error!(
                "<{} as Tokenizer>::tokenize panic: {}",
                std::any::type_name::<T>(),
                panic_err_to_str(&msg)
            );
            SQLITE_ERROR
        }
    }
}

fn panic_err_to_str(msg: &Box<dyn std::any::Any + Send>) -> &str {
    if let Some(msg) = msg.downcast_ref::<String>() {
        msg.as_str()
    } else if let Some(msg) = msg.downcast_ref::<&'static str>() {
        msg
    } else {
        "<non-string panic reason>"
    }
}

#[derive(Debug)]
pub enum RegisterTokenizerError {
    SelectFts5Failed,
    Fts5ApiNul,
    Fts5ApiVersionTooLow,
    Fts5xCreateTokenizerV2Nul,
    Fts5xCreateTokenizerFailed(i32),
}

impl std::fmt::Display for RegisterTokenizerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RegisterTokenizerError::SelectFts5Failed => {
                write!(f, "SELECT fts5(?1) failed.")
            }
            RegisterTokenizerError::Fts5ApiNul => {
                write!(f, "Could not get fts5 api.")
            }
            RegisterTokenizerError::Fts5ApiVersionTooLow => {
                write!(f, "The version of fts5 api is too low.")
            }
            RegisterTokenizerError::Fts5xCreateTokenizerV2Nul => {
                write!(f, "Fts5 api xCreateTokenizer_v2 ptr is null.")
            }
            RegisterTokenizerError::Fts5xCreateTokenizerFailed(rc) => {
                write!(
                    f,
                    "Fts5 xCreateTokenizer failed, the error flag when sqlite returned is {rc}."
                )
            }
        }
    }
}

impl std::error::Error for RegisterTokenizerError {}

/// 内部获取 fts5_api 指针的方法
unsafe fn get_fts5_api(db: &Connection) -> Result<*mut fts5_api, RegisterTokenizerError> {
    // 获取 fts5_api 结构体的指针，并且使用 sqlite3_bind_pointer 绑定指针
    // 详情 https://sqlite.org/fts5.html#extending_fts5
    let dbp = unsafe { db.handle() };
    let mut api: *mut fts5_api = std::ptr::null_mut();
    let mut stmt: *mut sqlite3_stmt = std::ptr::null_mut();
    const FTS5_QUERY_STATEMENT: &CStr = c"SELECT fts5(?1)";
    const FTS5_QUERY_STATEMENT_LEN: c_int = FTS5_QUERY_STATEMENT.count_bytes() as c_int;
    unsafe {
        if sqlite3_prepare_v3(
            dbp,
            FTS5_QUERY_STATEMENT.as_ptr(),
            FTS5_QUERY_STATEMENT_LEN,
            SQLITE_PREPARE_PERSISTENT,
            &mut stmt,
            std::ptr::null_mut(),
        ) != SQLITE_OK
        {
            return Err(RegisterTokenizerError::SelectFts5Failed);
        }
        sqlite3_bind_pointer(
            stmt,
            1,
            (&mut api) as *mut _ as *mut c_void,
            c"fts5_api_ptr".as_ptr(),
            None,
        );
        sqlite3_step(stmt);
        sqlite3_finalize(stmt);
    }
    if api.is_null() {
        return Err(RegisterTokenizerError::Fts5ApiNul);
    }
    Ok(api)
}

/// 注册 Tokenizer
pub fn register_tokenizer<T: Tokenizer>(
    db: &Connection,
    global_data: T::Global,
) -> Result<(), RegisterTokenizerError> {
    unsafe {
        let api: *mut fts5_api = get_fts5_api(db)?;
        let global_data = Box::into_raw(Box::new(global_data));
        if (*api).iVersion < FTS5_API_VERSION {
            return Err(RegisterTokenizerError::Fts5ApiVersionTooLow);
        }
        // 注册tokenizer
        let rc = ((*api)
            .xCreateTokenizer_v2
            .as_ref()
            .ok_or(RegisterTokenizerError::Fts5xCreateTokenizerV2Nul)?)(
            api,
            T::name().as_ptr(),
            global_data.cast::<c_void>(),
            &mut fts5_tokenizer_v2 {
                iVersion: FTS5_TOKENIZER_VERSION,
                xCreate: Some(x_create::<T>),
                xDelete: Some(x_delete::<T>),
                xTokenize: Some(x_tokenize::<T>),
            },
            Some(x_destroy::<T>),
        );
        if rc != SQLITE_OK {
            return Err(RegisterTokenizerError::Fts5xCreateTokenizerFailed(rc));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::tokenizer::jieba_tokenizer::JiebaTokenizer;
    use crate::tokenizer::register_tokenizer;
    use crate::tokenizer::simple_tokenizer::SimpleTokenizer;
    use rusqlite::Connection;

    #[test]
    fn test_register_simple_tokenizer_with_pinyin() {
        let conn = Connection::open_in_memory().unwrap();
        register_tokenizer::<SimpleTokenizer>(&conn, ()).unwrap();
        // 创建一个测试表
        conn.execute(
            "CREATE VIRTUAL TABLE t1 USING fts5(text, tokenize = 'simple');",
            [],
        )
        .unwrap();
        // 插入数据
        conn.execute(
            r#"INSERT INTO t1(text) VALUES ('中华人民共和国国歌'),('静夜思'),('国家'),('铁锅'),('举头望明月'),('like'),('liking'),('liked'),('I''m making a sqlite tokenizer'),('I''m learning English');"#,
            [],
        )
            .unwrap();
        // 使用 pinyin 分词，那么在查询的时候 match '国'，'国' 被处理成 guo，那么匹配结果包含 guo 这个读音的所有文档
        let mut stmt = conn
            .prepare("SELECT * FROM t1 WHERE text MATCH '国';")
            .unwrap();
        let result = stmt
            .query_map([], |row| Ok(row.get::<_, String>(0).unwrap()))
            .unwrap();
        let mut vec = Vec::new();
        for row in result {
            let row = row.unwrap();
            vec.push(row)
        }
        assert_eq!(["中华人民共和国国歌", "国家", "铁锅"], vec.as_slice());
    }

    #[test]
    fn test_register_simple_tokenizer_no_with_pinyin() {
        let conn = Connection::open_in_memory().unwrap();
        register_tokenizer::<SimpleTokenizer>(&conn, ()).unwrap();
        // 创建一个测试表, simple 不开启 pinyin 分词
        conn.execute(
            "CREATE VIRTUAL TABLE t1 USING fts5(text, tokenize = 'simple disable_pinyin');",
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
            .prepare("SELECT * FROM t1 WHERE text MATCH '国';")
            .unwrap();
        let result = stmt
            .query_map([], |row| Ok(row.get::<_, String>(0).unwrap()))
            .unwrap();
        let mut vec = Vec::new();
        for row in result {
            let row = row.unwrap();
            vec.push(row)
        }
        // 不开启开启拼音分词，这里字符正常
        assert_eq!(["中华人民共和国国歌", "国家"], vec.as_slice());
        let mut stmt = conn
            .prepare("SELECT * FROM t1 WHERE text MATCH 'like'")
            .unwrap();
        let result = stmt
            .query_map([], |row| Ok(row.get::<_, String>(0).unwrap()))
            .unwrap();
        let mut vec = Vec::new();
        for row in result {
            let row = row.unwrap();
            vec.push(row);
        }
        // like 在停词表中，查询结果为空
        assert_eq!(0, vec.len());
        let mut stmt = conn
            .prepare("SELECT * FROM t1 WHERE text MATCH 'tokenizer'")
            .unwrap();
        let result = stmt
            .query_map([], |row| Ok(row.get::<_, String>(0).unwrap()))
            .unwrap();
        let mut vec = Vec::new();
        for row in result {
            let row = row.unwrap();
            vec.push(row);
        }
        assert_eq!(["I'm making a sqlite tokenizer"], vec.as_slice());
    }

    #[test]
    fn test_register_simple_tokenizer_no_with_stopword() {
        let conn = Connection::open_in_memory().unwrap();
        register_tokenizer::<SimpleTokenizer>(&conn, ()).unwrap();
        // 创建一个测试表, simple 不启用停词表
        conn.execute(
            "CREATE VIRTUAL TABLE t1 USING fts5(text, tokenize = 'simple disable_stopword');",
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
            .prepare("SELECT * FROM t1 WHERE text MATCH 'like'")
            .unwrap();
        let result = stmt
            .query_map([], |row| Ok(row.get::<_, String>(0).unwrap()))
            .unwrap();
        let mut vec = Vec::new();
        for row in result {
            let row = row.unwrap();
            vec.push(row);
        }
        assert_eq!(["like", "liking", "liked"], vec.as_slice());
    }

    #[test]
    fn test_register_jieba_tokenizer() {
        let conn = Connection::open_in_memory().unwrap();
        register_tokenizer::<JiebaTokenizer>(&conn, ()).unwrap();
        // 创建一个测试表, simple 不开启 pinyin 分词
        conn.execute(
            "CREATE VIRTUAL TABLE t1 USING fts5(text, tokenize = 'jieba');",
            [],
        )
        .unwrap();
        // 插入数据
        conn.execute(
            "INSERT INTO t1(text) VALUES ('中华人民共和国国歌'),('静夜思'),('国家'),('举头望明月'),('like'),('liking'),('liked');",
            [],
        )
            .unwrap();
        let mut stmt = conn
            .prepare("SELECT * FROM t1 WHERE text MATCH '国家';")
            .unwrap();
        let result = stmt
            .query_map([], |row| Ok(row.get::<_, String>(0).unwrap()))
            .unwrap();
        for row in result {
            let row = row.unwrap();
            assert_eq!("国家".to_owned(), row);
        }
    }

    #[test]
    fn test_register_jieba_tokenizer_with_space_str() {
        let conn = Connection::open_in_memory().unwrap();
        register_tokenizer::<JiebaTokenizer>(&conn, ()).unwrap();
        // 创建一个测试表, simple 不开启 pinyin 分词
        conn.execute(
            "CREATE VIRTUAL TABLE t1 USING fts5(id, title, introduction, summary, readme, tokenize = 'jieba');",
            [],
        )
            .unwrap();
        // 插入数据
        conn.execute("INSERT INTO t1(id, title, introduction, summary, readme) VALUES ('019976ba-beff-7311-96c9-d0ba0bce1e1b', 'codeup', 'Great code generation model based on Llama2.', '', '');", [])
            .unwrap();
    }

    #[test]
    fn test_register_jieba_tokenizer_with_() {
        let conn = Connection::open_in_memory().unwrap();
        register_tokenizer::<JiebaTokenizer>(&conn, ()).unwrap();
        // 创建一个测试表
        conn.execute(
            "CREATE VIRTUAL TABLE t1 USING fts5(text, tokenize = 'jieba');",
            [],
        )
        .unwrap();
        // 插入数据
        conn.execute(r#"INSERT INTO t1(text) VALUES ('社会主义国家'),('静夜思'),('国家'),('举头望明月'),('like'),('liking'),('liked'),('I''m making a sqlite tokenizer'),('I''m learning English');"#, []).unwrap();
        // 查询
        let mut stmt = conn
            .prepare("SELECT * FROM t1 WHERE text MATCH '国家';")
            .unwrap();
        // 结果处理
        let result = stmt
            .query_map([], |row| Ok(row.get::<_, String>(0).unwrap()))
            .unwrap();
        let mut vec = Vec::new();
        for row in result {
            let row = row.unwrap();
            vec.push(row)
        }
        assert_eq!(["社会主义国家", "国家"], vec.as_slice());
    }
}
