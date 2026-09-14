//! 单文件 sqlite 的打开与建表(表名/列名对齐裁判 Room schema,便于 Phase 4
//! 导入 legado 备份)。
//!
//! 与 [`crate::CookieStore`] 各自持一条 `Connection` 指向同一个文件 ——
//! Room 侧也是多 DAO 共库;写入串行由 engine 侧的 Mutex + `busy_timeout` 保证。

use rusqlite::{Connection, Row, params};
use serde_json::{Map, Value};

/// `books` / `chapters` / `caches` / `highlights` / `book_sources` 的建表 DDL。
///
/// books/chapters/caches/highlights 列名逐字对齐 judge/engine 的 Room 实体
/// (Book.kt / BookChapter.kt / Cache.kt / BookHighlight.kt);本 Phase 用不到的列
/// 照样建出来并给默认值,免得日后导入备份时缺列。`book_sources` 是例外:书源本来就以 JSON 流通
/// (导入/分享都是),故只把排序/过滤要用的三列拆出来,其余整串存 `json`
/// —— 导入备份走的也是 JSON 导入那条路,不需要列对齐。
pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS books (
    bookUrl TEXT NOT NULL PRIMARY KEY DEFAULT '',
    tocUrl TEXT NOT NULL DEFAULT '',
    origin TEXT NOT NULL DEFAULT 'loc_book',
    originName TEXT NOT NULL DEFAULT '',
    name TEXT NOT NULL DEFAULT '',
    author TEXT NOT NULL DEFAULT '',
    kind TEXT,
    customTag TEXT,
    coverUrl TEXT,
    customCoverUrl TEXT,
    intro TEXT,
    customIntro TEXT,
    charset TEXT,
    type INTEGER NOT NULL DEFAULT 0,
    "group" INTEGER NOT NULL DEFAULT 0,
    latestChapterTitle TEXT,
    latestChapterTime INTEGER NOT NULL DEFAULT 0,
    lastCheckTime INTEGER NOT NULL DEFAULT 0,
    lastCheckCount INTEGER NOT NULL DEFAULT 0,
    totalChapterNum INTEGER NOT NULL DEFAULT 0,
    durChapterTitle TEXT,
    durChapterIndex INTEGER NOT NULL DEFAULT 0,
    durChapterPos INTEGER NOT NULL DEFAULT 0,
    durChapterTime INTEGER NOT NULL DEFAULT 0,
    wordCount TEXT,
    canUpdate INTEGER NOT NULL DEFAULT 1,
    "order" INTEGER NOT NULL DEFAULT 0,
    originOrder INTEGER NOT NULL DEFAULT 0,
    variable TEXT,
    readConfig TEXT,
    syncTime INTEGER NOT NULL DEFAULT 0,
    persistedCoverUrl TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS index_books_name_author ON books (name, author);

CREATE TABLE IF NOT EXISTS chapters (
    url TEXT NOT NULL DEFAULT '',
    title TEXT NOT NULL DEFAULT '',
    isVolume INTEGER NOT NULL DEFAULT 0,
    baseUrl TEXT NOT NULL DEFAULT '',
    bookUrl TEXT NOT NULL DEFAULT '',
    "index" INTEGER NOT NULL DEFAULT 0,
    isVip INTEGER NOT NULL DEFAULT 0,
    isPay INTEGER NOT NULL DEFAULT 0,
    resourceUrl TEXT,
    tag TEXT,
    wordCount TEXT,
    start INTEGER,
    end INTEGER,
    startFragmentId TEXT,
    endFragmentId TEXT,
    variable TEXT,
    imgUrl TEXT,
    PRIMARY KEY (url, bookUrl)
);
CREATE INDEX IF NOT EXISTS index_chapters_bookUrl ON chapters (bookUrl);
CREATE UNIQUE INDEX IF NOT EXISTS index_chapters_bookUrl_index ON chapters (bookUrl, "index");

CREATE TABLE IF NOT EXISTS caches (
    key TEXT NOT NULL PRIMARY KEY DEFAULT '',
    value TEXT,
    deadline INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS highlights (
    time INTEGER NOT NULL PRIMARY KEY DEFAULT 0,
    bookUrl TEXT NOT NULL DEFAULT '',
    chapterUrl TEXT NOT NULL DEFAULT '',
    bookName TEXT NOT NULL DEFAULT '',
    bookAuthor TEXT NOT NULL DEFAULT '',
    chapterIndex INTEGER NOT NULL DEFAULT 0,
    chapterPos INTEGER NOT NULL DEFAULT 0,
    chapterPosEnd INTEGER NOT NULL DEFAULT 0,
    layoutTitleLength INTEGER NOT NULL DEFAULT -1,
    chapterName TEXT NOT NULL DEFAULT '',
    bookText TEXT NOT NULL DEFAULT '',
    style TEXT NOT NULL DEFAULT '',
    note TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS index_highlights_bookUrl ON highlights (bookUrl);

CREATE TABLE IF NOT EXISTS book_sources (
    bookSourceUrl TEXT NOT NULL PRIMARY KEY DEFAULT '',
    enabled INTEGER NOT NULL DEFAULT 1,
    customOrder INTEGER NOT NULL DEFAULT 0,
    json TEXT NOT NULL DEFAULT '{}'
);
"#;

/// 打开(必要时创建)库文件并建表。`path` 为 `:memory:` 时是内存库。
pub fn open(path: &str) -> rusqlite::Result<Connection> {
    let conn =
        if path == ":memory:" { Connection::open_in_memory()? } else { Connection::open(path)? };
    prepare(&conn)?;
    Ok(conn)
}

/// 建表 + 连接级 pragma(多连接共库时必须给 busy_timeout)。
pub fn prepare(conn: &Connection) -> rusqlite::Result<()> {
    tune(conn)?;
    // `book_sources` 曾是 29 列的 Room 镜像表(2026-08 前),后来换成
    // 「三列 + 原始 JSON」。旧行要在事务内搬迁:预览版同样会持久化
    // 用户已导入的书源,普通 open 不能把它们当临时数据删掉。
    let old_shape: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'book_sources')
             AND NOT EXISTS(SELECT 1 FROM pragma_table_info('book_sources') WHERE name = 'json')",
        [],
        |r| r.get(0),
    )?;
    if old_shape {
        migrate_legacy_sources(conn)?;
    }
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

const LEGACY_SOURCE_SELECT: &str = "SELECT bookSourceUrl, bookSourceName, bookSourceGroup,
     bookSourceType, bookUrlPattern, customOrder, enabled, enabledExplore, jsLib,
     enabledCookieJar, concurrentRate, header, loginUrl, loginCheckJs, coverDecodeJs,
     mainJs, bookSourceComment, variableComment, lastUpdateTime, respondTime, weight,
     exploreUrl, exploreScreen, ruleExplore, searchUrl, ruleSearch, ruleBookInfo,
     ruleToc, ruleContent FROM book_sources";

/// 29 列旧表 → 「三列 + JSON」。整段在同一个事务里,建表或回写
/// 失败时 `DROP` 也会回滚,旧书源仍在。
fn migrate_legacy_sources(conn: &Connection) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    let rows = {
        let mut st = tx.prepare(LEGACY_SOURCE_SELECT)?;
        st.query_map([], legacy_source_row)?.collect::<rusqlite::Result<Vec<_>>>()?
    };

    tx.execute("DROP TABLE book_sources", [])?;
    tx.execute_batch(SCHEMA)?;
    {
        let mut st = tx.prepare(
            "INSERT INTO book_sources (bookSourceUrl, enabled, customOrder, json)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        for (url, enabled, custom_order, json) in rows {
            st.execute(params![url, enabled, custom_order, json])?;
        }
    }
    tx.commit()
}

fn legacy_source_row(r: &Row<'_>) -> rusqlite::Result<(String, bool, i64, String)> {
    let url: String = r.get(0)?;
    let enabled: bool = r.get(6)?;
    let custom_order: i64 = r.get(5)?;
    let mut m = Map::new();
    m.insert("bookSourceUrl".into(), Value::String(url.clone()));
    m.insert("bookSourceName".into(), Value::String(r.get(1)?));
    for (key, column) in [
        ("bookSourceGroup", 2),
        ("bookUrlPattern", 4),
        ("jsLib", 8),
        ("concurrentRate", 10),
        ("header", 11),
        ("loginUrl", 12),
        ("loginCheckJs", 13),
        ("coverDecodeJs", 14),
        ("mainJs", 15),
        ("bookSourceComment", 16),
        ("variableComment", 17),
        ("exploreUrl", 21),
        ("exploreScreen", 22),
        ("ruleExplore", 23),
        ("searchUrl", 24),
        ("ruleSearch", 25),
        ("ruleBookInfo", 26),
        ("ruleToc", 27),
        ("ruleContent", 28),
    ] {
        if let Some(value) = r.get::<_, Option<String>>(column)? {
            m.insert(key.into(), Value::String(value));
        }
    }
    m.insert("bookSourceType".into(), Value::from(r.get::<_, i64>(3)?));
    m.insert("customOrder".into(), Value::from(custom_order));
    m.insert("enabled".into(), Value::Bool(enabled));
    m.insert("enabledExplore".into(), Value::Bool(r.get(7)?));
    if let Some(value) = r.get::<_, Option<bool>>(9)? {
        m.insert("enabledCookieJar".into(), Value::Bool(value));
    }
    m.insert("lastUpdateTime".into(), Value::from(r.get::<_, i64>(18)?));
    m.insert("respondTime".into(), Value::from(r.get::<_, i64>(19)?));
    m.insert("weight".into(), Value::from(r.get::<_, i64>(20)?));
    Ok((url, enabled, custom_order, Value::Object(m).to_string()))
}

/// 连接级 pragma,**每一条指向共享库文件的连接都要走**(包括
/// [`crate::CookieStore`] 自己那条——漏掉它,并发搜索撞 `SQLITE_BUSY`
/// 时 cookie 写入会直接失败)。
pub fn tune(conn: &Connection) -> rusqlite::Result<()> {
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;").ok();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY_SOURCE_SCHEMA: &str = r#"
CREATE TABLE book_sources (
    bookSourceUrl TEXT NOT NULL PRIMARY KEY DEFAULT '', bookSourceName TEXT NOT NULL DEFAULT '',
    bookSourceGroup TEXT, bookSourceType INTEGER NOT NULL DEFAULT 0, bookUrlPattern TEXT,
    customOrder INTEGER NOT NULL DEFAULT 0, enabled INTEGER NOT NULL DEFAULT 1,
    enabledExplore INTEGER NOT NULL DEFAULT 1, jsLib TEXT, enabledCookieJar INTEGER DEFAULT 0,
    concurrentRate TEXT, header TEXT, loginUrl TEXT, loginCheckJs TEXT, coverDecodeJs TEXT,
    mainJs TEXT, bookSourceComment TEXT, variableComment TEXT,
    lastUpdateTime INTEGER NOT NULL DEFAULT 0, respondTime INTEGER NOT NULL DEFAULT 180000,
    weight INTEGER NOT NULL DEFAULT 0, exploreUrl TEXT, exploreScreen TEXT, ruleExplore TEXT,
    searchUrl TEXT, ruleSearch TEXT, ruleBookInfo TEXT, ruleToc TEXT, ruleContent TEXT
);
"#;

    #[test]
    fn legacy_sources_are_migrated_without_data_loss() {
        let conn = Connection::open_in_memory().expect("open");
        conn.execute_batch(LEGACY_SOURCE_SCHEMA).expect("legacy schema");
        conn.execute(
            "INSERT INTO book_sources (
                bookSourceUrl, bookSourceName, customOrder, enabled, header, ruleSearch
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "https://legacy.example",
                "旧书源",
                7,
                false,
                r#"{"User-Agent":"legacy"}"#,
                r#"{"bookList":"@css:li","name":"@css:h1@text"}"#,
            ],
        )
        .expect("seed");

        prepare(&conn).expect("migrate");

        let (enabled, order, json): (bool, i64, String) = conn
            .query_row(
                "SELECT enabled, customOrder, json FROM book_sources WHERE bookSourceUrl = ?1",
                ["https://legacy.example"],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("migrated row");
        let raw: Value = serde_json::from_str(&json).expect("json");
        assert!(!enabled);
        assert_eq!(order, 7);
        assert_eq!(raw["bookSourceName"], "旧书源");
        assert_eq!(raw["header"], r#"{"User-Agent":"legacy"}"#);
        assert_eq!(raw["ruleSearch"], r#"{"bookList":"@css:li","name":"@css:h1@text"}"#);
    }
}
