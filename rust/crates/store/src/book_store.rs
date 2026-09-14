//! 书架(`books`)、目录(`chapters`)与正文缓存(`caches`)的落库面。
//!
//! 表结构见 [`crate::db::SCHEMA`]。`rubato_core::entities::Book` 只带流水线
//! 用得到的字段面(它被差分钉住,不动它),阅读进度/时间戳这些**只属于产品侧**
//! 的列放在 [`BookRow`] 的外层。

use crate::db;
use crate::highlight_store::HighlightStore;
use rubato_core::entities::{Book, BookChapter, EntityVars};
use rusqlite::{Connection, OptionalExtension, Row, params};

/// 一行 `books`:流水线实体 + 产品侧列
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BookRow {
    pub book: Book,
    /// 当前阅读进度(章内首行字符索引)
    pub dur_chapter_pos: i32,
    /// 最近一次打开正文的时间(毫秒)
    pub dur_chapter_time: i64,
    /// 最近一次更新书籍信息的时间(毫秒)
    pub last_check_time: i64,
    /// 最新章节标题的更新时间(毫秒)
    pub latest_chapter_time: i64,
    /// 书架手动排序
    pub order: i32,
}

impl BookRow {
    pub fn new(book: Book) -> BookRow {
        BookRow { book, ..Default::default() }
    }
}

pub struct BookStore {
    conn: Connection,
}

impl BookStore {
    pub fn open(path: &str) -> rusqlite::Result<BookStore> {
        Ok(BookStore { conn: db::open(path)? })
    }

    /// 同一个库文件上的另一个落库面([`crate::HighlightStore`])借它写。
    ///
    /// **不为它另开一条连接**:写入串行由 engine 侧那把 Mutex 保证,多开一条
    /// 只是多一个 `busy_timeout` 的竞争者;而且「移出书架时批注跟着走」要与
    /// 删书在同一个事务里 —— 两条连接做不到。
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn open_in_memory() -> rusqlite::Result<BookStore> {
        BookStore::open(":memory:")
    }

    // ---------------- books ----------------

    /// `bookDao.insert`(Room `@Insert(onConflict = REPLACE)`)
    pub fn save_book(&mut self, row: &BookRow) -> rusqlite::Result<()> {
        let b = &row.book;
        self.conn.execute(
            "INSERT OR REPLACE INTO books (
                bookUrl, tocUrl, origin, originName, name, author, kind, coverUrl, intro,
                type, latestChapterTitle, latestChapterTime, lastCheckTime, lastCheckCount,
                totalChapterNum, durChapterTitle, durChapterIndex, durChapterPos,
                durChapterTime, wordCount, \"order\", originOrder, variable
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18,
                ?19, ?20, ?21, ?22, ?23
            )",
            params![
                b.book_url,
                b.toc_url,
                b.origin,
                b.origin_name,
                b.name,
                b.author,
                b.kind,
                b.cover_url,
                b.intro,
                b.type_,
                b.latest_chapter_title,
                row.latest_chapter_time,
                row.last_check_time,
                b.last_check_count,
                b.total_chapter_num,
                b.dur_chapter_title,
                b.dur_chapter_index,
                row.dur_chapter_pos,
                row.dur_chapter_time,
                b.word_count,
                row.order,
                b.origin_order,
                vars_to_variable(&b.vars),
            ],
        )?;
        Ok(())
    }

    pub fn get_book(&self, book_url: &str) -> rusqlite::Result<Option<BookRow>> {
        self.conn
            .query_row(&format!("{BOOK_SELECT} WHERE bookUrl = ?1"), params![book_url], book_row)
            .optional()
    }

    /// 书架列表:手动序 → 最近阅读时间倒序
    pub fn list_books(&self) -> rusqlite::Result<Vec<BookRow>> {
        let mut st = self
            .conn
            .prepare(&format!("{BOOK_SELECT} ORDER BY \"order\" ASC, durChapterTime DESC"))?;
        let rows = st.query_map([], book_row)?;
        rows.collect()
    }

    pub fn delete_book(&mut self, book_url: &str) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM chapters WHERE bookUrl = ?1", params![book_url])?;
        tx.execute("DELETE FROM books WHERE bookUrl = ?1", params![book_url])?;
        // 批注挂在书上,留着就是一堆再也打不开的孤儿(计划书 M4c)
        HighlightStore::delete_book(&tx, book_url)?;
        tx.execute(
            "DELETE FROM caches WHERE key LIKE ?1 ESCAPE '\\'",
            params![format!("{}:%", like_escape(&content_key_prefix(book_url)))],
        )?;
        tx.commit()
    }

    /// 只写进度三列(翻页热路径,不覆盖书籍信息)
    pub fn save_progress(
        &mut self,
        book_url: &str,
        index: i32,
        pos: i32,
        title: Option<&str>,
        time_ms: i64,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE books SET durChapterIndex = ?2, durChapterPos = ?3,
                 durChapterTitle = ?4, durChapterTime = ?5 WHERE bookUrl = ?1",
            params![book_url, index, pos, title, time_ms],
        )?;
        Ok(())
    }

    // ---------------- chapters ----------------

    /// `bookChapterDao.delByBook` + `insert`:目录整体替换(一个事务)
    pub fn save_chapters(
        &mut self,
        book_url: &str,
        chapters: &[BookChapter],
    ) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM chapters WHERE bookUrl = ?1", params![book_url])?;
        {
            let mut st = tx.prepare(
                "INSERT OR REPLACE INTO chapters (
                    url, title, isVolume, baseUrl, bookUrl, \"index\",
                    isVip, isPay, tag, wordCount, variable
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            for c in chapters {
                st.execute(params![
                    c.url,
                    c.title,
                    c.is_volume,
                    c.base_url,
                    book_url,
                    c.index,
                    c.is_vip,
                    c.is_pay,
                    c.tag,
                    c.word_count,
                    vars_to_variable(&c.vars),
                ])?;
            }
        }
        tx.execute(
            "UPDATE books SET totalChapterNum = ?2 WHERE bookUrl = ?1",
            params![book_url, chapters.len() as i32],
        )?;
        tx.commit()
    }

    pub fn list_chapters(&self, book_url: &str) -> rusqlite::Result<Vec<BookChapter>> {
        let mut st = self
            .conn
            .prepare(&format!("{CHAPTER_SELECT} WHERE bookUrl = ?1 ORDER BY \"index\" ASC"))?;
        let rows = st.query_map(params![book_url], chapter_row)?;
        rows.collect()
    }

    pub fn get_chapter(&self, book_url: &str, index: i32) -> rusqlite::Result<Option<BookChapter>> {
        self.conn
            .query_row(
                &format!("{CHAPTER_SELECT} WHERE bookUrl = ?1 AND \"index\" = ?2"),
                params![book_url, index],
                chapter_row,
            )
            .optional()
    }

    // ---------------- 正文缓存(caches)----------------

    /// 键形态 `content:<bookUrl>:<chapterUrl>`;落的就是通用 kv 那条路
    /// (`deadline = 0` = 不过期,对齐 CacheManager 的 deadline 语义)
    pub fn put_content(
        &mut self,
        book_url: &str,
        chapter_url: &str,
        content: &str,
    ) -> rusqlite::Result<()> {
        self.put_cache(&content_key(book_url, chapter_url), content)
    }

    pub fn get_content(
        &self,
        book_url: &str,
        chapter_url: &str,
    ) -> rusqlite::Result<Option<String>> {
        self.get_cache(&content_key(book_url, chapter_url))
    }

    /// 这本书**已经缓存了正文的那些章节地址**。
    ///
    /// 一次查询把整本书的 `content:` 前缀捞出来 —— 目录页要按章标「已缓存 /
    /// 未缓存」,一章一次 `get_content` 是几百次查询。键形态见 [`content_key`],
    /// 前缀之后剩下的就是 chapterUrl(它自己可能含 `:`,所以按**第一个**冒号切)。
    pub fn cached_chapter_urls(
        &self,
        book_url: &str,
    ) -> rusqlite::Result<std::collections::HashSet<String>> {
        let prefix = format!("{}:", content_key_prefix(book_url));
        let mut st = self
            .conn
            .prepare("SELECT key FROM caches WHERE key LIKE ?1 ESCAPE '\\'")?;
        let rows = st.query_map(params![format!("{}%", like_escape(&prefix))], |r| {
            r.get::<_, String>(0)
        })?;
        let mut out = std::collections::HashSet::new();
        for k in rows {
            let k = k?;
            if let Some(rest) = k.strip_prefix(&prefix) {
                out.insert(rest.to_string());
            }
        }
        Ok(out)
    }

    // ---------------- 通用 kv(仍是 `caches` 表,CacheManager 的语义) ----------------
    //
    // 正文缓存走 `content:` 前缀,JS 宿主的 CacheManager 走 `js:`,
    // 设备级常量走 `env:` —— 同一张表,前缀分家。

    /// 单条读。`deadline = 0` = 不过期(对齐 CacheManager)。
    pub fn get_cache(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM caches WHERE key = ?1 AND (deadline = 0 OR deadline > ?2)",
                params![key, now_ms()],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()
            .map(Option::flatten)
    }

    /// 单条写(不过期)
    pub fn put_cache(&mut self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO caches (key, value, deadline) VALUES (?1, ?2, 0)",
            params![key, value],
        )?;
        Ok(())
    }

    /// 按前缀列出(返回的键**去掉前缀**)
    pub fn list_cache_prefix(&self, prefix: &str) -> rusqlite::Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare(
            "SELECT key, value FROM caches WHERE key LIKE ?1 ESCAPE '\\' \
             AND (deadline = 0 OR deadline > ?2)",
        )?;
        let rows = st.query_map(params![format!("{}%", like_escape(prefix)), now_ms()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default()))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (k, v) = row?;
            out.push((k[prefix.len()..].to_string(), v));
        }
        Ok(out)
    }

    /// **按差量写回**某个前缀下的条目(键传的是去掉前缀的那一半),一次事务。
    ///
    /// 不能整段 DELETE+INSERT:并发搜索时 6 个 worker 各自 seed 一份快照、
    /// 各自收工,**读-改-写不是原子的**,整段替换会让后收工的那条线程把
    /// 别的源刚写下的 `sourceVariable_*` 一并抹掉(它 seed 的时候还看不见)。
    /// 差量写只碰自己动过的键,跨源就不会互相踩。
    ///
    /// `upserts` 是新增/改过的,`deletes` 是 JS 里 `remove` 掉的
    /// —— 「remove 掉的条目下次不该再被 seed 回来」靠后者。
    pub fn update_cache_prefix(
        &mut self,
        prefix: &str,
        upserts: &[(String, String)],
        deletes: &[String],
    ) -> rusqlite::Result<()> {
        if upserts.is_empty() && deletes.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        {
            let mut del = tx.prepare("DELETE FROM caches WHERE key = ?1")?;
            for k in deletes {
                del.execute(params![format!("{prefix}{k}")])?;
            }
            let mut ins = tx.prepare(
                "INSERT OR REPLACE INTO caches (key, value, deadline) VALUES (?1, ?2, 0)",
            )?;
            for (k, v) in upserts {
                ins.execute(params![format!("{prefix}{k}"), v])?;
            }
        }
        tx.commit()
    }
}

fn content_key_prefix(book_url: &str) -> String {
    format!("content:{book_url}")
}

fn content_key(book_url: &str, chapter_url: &str) -> String {
    format!("content:{book_url}:{chapter_url}")
}

/// LIKE 模式里的元字符转义(书链接可能带 `%`/`_`)
fn like_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// `variableMap` → `variable` 列。键排序,保证同一份变量落库字节稳定。
fn vars_to_variable(vars: &EntityVars) -> Option<String> {
    if let Some(raw) = &vars.raw_unparsed {
        return Some(raw.clone());
    }
    if !vars.present {
        return None;
    }
    let mut keys: Vec<&String> = vars.map.keys().collect();
    keys.sort();
    let mut obj = serde_json::Map::new();
    for k in keys {
        obj.insert(k.clone(), serde_json::Value::String(vars.map[k].clone()));
    }
    Some(serde_json::Value::Object(obj).to_string())
}

const BOOK_SELECT: &str = "SELECT bookUrl, tocUrl, origin, originName, name, author, kind,
     coverUrl, intro, type, latestChapterTitle, latestChapterTime, lastCheckTime,
     lastCheckCount, totalChapterNum, durChapterTitle, durChapterIndex, durChapterPos,
     durChapterTime, wordCount, \"order\", originOrder, variable FROM books";

fn book_row(r: &Row<'_>) -> rusqlite::Result<BookRow> {
    let variable: Option<String> = r.get(22)?;
    Ok(BookRow {
        book: Book {
            book_url: r.get(0)?,
            toc_url: r.get(1)?,
            origin: r.get(2)?,
            origin_name: r.get(3)?,
            name: r.get(4)?,
            author: r.get(5)?,
            kind: r.get(6)?,
            cover_url: r.get(7)?,
            intro: r.get(8)?,
            type_: r.get(9)?,
            latest_chapter_title: r.get(10)?,
            last_check_count: r.get(13)?,
            total_chapter_num: r.get(14)?,
            dur_chapter_title: r.get(15)?,
            dur_chapter_index: r.get(16)?,
            word_count: r.get(19)?,
            origin_order: r.get(21)?,
            vars: EntityVars::from_variable(variable.as_deref()),
            info_html: None,
            toc_html: None,
            download_urls: None,
            config_fixed_type: false,
        },
        latest_chapter_time: r.get(11)?,
        last_check_time: r.get(12)?,
        dur_chapter_pos: r.get(17)?,
        dur_chapter_time: r.get(18)?,
        order: r.get(20)?,
    })
}

const CHAPTER_SELECT: &str = "SELECT url, title, isVolume, baseUrl, bookUrl, \"index\",
     isVip, isPay, tag, wordCount, variable FROM chapters";

fn chapter_row(r: &Row<'_>) -> rusqlite::Result<BookChapter> {
    let variable: Option<String> = r.get(10)?;
    Ok(BookChapter {
        url: r.get(0)?,
        title: r.get(1)?,
        is_volume: r.get(2)?,
        base_url: r.get(3)?,
        book_url: r.get(4)?,
        index: r.get(5)?,
        is_vip: r.get(6)?,
        is_pay: r.get(7)?,
        tag: r.get(8)?,
        word_count: r.get(9)?,
        vars: EntityVars::from_variable(variable.as_deref()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 通用 kv:`js:` 段是书源 JS 的 CacheManager,产品侧每次调用 seed / flush。
    /// 关键是 `update_cache_prefix` 的 `deletes` 要**真的删掉** —— JS 里 remove
    /// 掉的条目不该在下次被 seed 回来,而 `content:` 段不能被它误删。
    #[test]
    fn js_cache_prefix_roundtrip() {
        let mut st = BookStore::open(":memory:").expect("open");
        st.put_content("http://a/1", "c1", "正文").expect("content");
        st.update_cache_prefix(
            "js:",
            &[
                ("loginHeader_http://s".to_string(), "{\"Cookie\":\"a=1\"}".to_string()),
                ("sourceVariable_http://s".to_string(), "v1".to_string()),
            ],
            &[],
        )
        .expect("update");
        let mut got = st.list_cache_prefix("js:").expect("list");
        got.sort();
        assert_eq!(
            got,
            vec![
                ("loginHeader_http://s".to_string(), "{\"Cookie\":\"a=1\"}".to_string()),
                ("sourceVariable_http://s".to_string(), "v1".to_string()),
            ]
        );
        // remove 掉的那条要真没了,改过的那条要落新值
        st.update_cache_prefix(
            "js:",
            &[("sourceVariable_http://s".to_string(), "v2".to_string())],
            &["loginHeader_http://s".to_string()],
        )
        .expect("update2");
        assert_eq!(
            st.list_cache_prefix("js:").expect("list2"),
            vec![("sourceVariable_http://s".to_string(), "v2".to_string())]
        );
        // 别的前缀不受牵连
        assert_eq!(st.get_content("http://a/1", "c1").expect("c"), Some("正文".to_string()));
    }

    /// **并发搜索的那个形状**:线程 B 在 A 落库之前 seed(所以快照里没有 A 的键),
    /// 之后 B 收工。从前是整段 DELETE+INSERT,B 会把 A 刚写下的键抹掉;
    /// 差量写只碰 B 自己动过的键,两边都留得住。
    #[test]
    fn js_cache_concurrent_flush_keeps_other_source() {
        let mut st = BookStore::open(":memory:").expect("open");
        // T0/T1:两条线程各自 seed(都是空的)
        let seed_a: Vec<(String, String)> = st.list_cache_prefix("js:").expect("seed a");
        let seed_b: Vec<(String, String)> = st.list_cache_prefix("js:").expect("seed b");
        assert!(seed_a.is_empty() && seed_b.is_empty());
        // T2:A 写自己的变量并收工
        st.update_cache_prefix("js:", &[("sourceVariable_a".to_string(), "va".to_string())], &[])
            .expect("flush a");
        // T3:B 收工 —— 它的快照里根本没有 sourceVariable_a
        st.update_cache_prefix("js:", &[("sourceVariable_b".to_string(), "vb".to_string())], &[])
            .expect("flush b");
        let mut got = st.list_cache_prefix("js:").expect("list");
        got.sort();
        assert_eq!(
            got,
            vec![
                ("sourceVariable_a".to_string(), "va".to_string()),
                ("sourceVariable_b".to_string(), "vb".to_string()),
            ]
        );
    }

    /// 设备级常量走 `env:` 段,单条读写
    #[test]
    fn env_cache_single_key() {
        let mut st = BookStore::open(":memory:").expect("open");
        assert_eq!(st.get_cache("env:androidId").expect("get"), None);
        st.put_cache("env:androidId", "0123456789abcdef").expect("put");
        assert_eq!(
            st.get_cache("env:androidId").expect("get2"),
            Some("0123456789abcdef".to_string())
        );
    }

    fn book(url: &str, name: &str) -> BookRow {
        BookRow::new(Book {
            book_url: url.into(),
            toc_url: url.into(),
            origin: "http://s".into(),
            origin_name: "源".into(),
            name: name.into(),
            author: "佚名".into(),
            ..Default::default()
        })
    }

    #[test]
    fn book_round_trip_and_progress() {
        let mut st = BookStore::open_in_memory().expect("open");
        let mut row = book("http://a/1", "书甲");
        row.book.vars.put("k", "v");
        st.save_book(&row).expect("save");
        let got = st.get_book("http://a/1").expect("get").expect("有");
        assert_eq!(got.book.name, "书甲");
        assert_eq!(got.book.vars.map.get("k").map(String::as_str), Some("v"));

        st.save_progress("http://a/1", 7, 120, Some("第八章"), 1234).expect("progress");
        let got = st.get_book("http://a/1").expect("get").expect("有");
        assert_eq!((got.book.dur_chapter_index, got.dur_chapter_pos), (7, 120));
        assert_eq!(got.book.dur_chapter_title.as_deref(), Some("第八章"));
        assert_eq!(got.dur_chapter_time, 1234);
    }

    #[test]
    fn chapters_replace_and_count() {
        let mut st = BookStore::open_in_memory().expect("open");
        st.save_book(&book("http://a/1", "书甲")).expect("save");
        let chs: Vec<BookChapter> = (0..3)
            .map(|i| BookChapter {
                url: format!("http://a/c{i}"),
                title: format!("第{i}章"),
                book_url: "http://a/1".into(),
                index: i,
                ..Default::default()
            })
            .collect();
        st.save_chapters("http://a/1", &chs).expect("chapters");
        assert_eq!(st.list_chapters("http://a/1").expect("count").len(), 3);
        assert_eq!(st.get_book("http://a/1").expect("g").expect("有").book.total_chapter_num, 3);
        // 重复保存是整体替换,不是追加
        st.save_chapters("http://a/1", &chs[..2]).expect("chapters");
        assert_eq!(st.list_chapters("http://a/1").expect("count").len(), 2);
        assert_eq!(st.get_chapter("http://a/1", 1).expect("g").expect("有").title, "第1章");
        assert!(st.get_chapter("http://a/1", 2).expect("g").is_none());
    }

    #[test]
    fn content_cache_and_cascade_delete() {
        let mut st = BookStore::open_in_memory().expect("open");
        st.save_book(&book("http://a/1", "书甲")).expect("save");
        st.save_chapters(
            "http://a/1",
            &[BookChapter {
                url: "http://a/c0".into(),
                book_url: "http://a/1".into(),
                ..Default::default()
            }],
        )
        .expect("chapters");
        st.put_content("http://a/1", "http://a/c0", "正文").expect("put");
        assert_eq!(
            st.get_content("http://a/1", "http://a/c0").expect("get").as_deref(),
            Some("正文")
        );

        st.delete_book("http://a/1").expect("del");
        assert!(st.get_book("http://a/1").expect("g").is_none());
        assert!(st.list_chapters("http://a/1").expect("count").is_empty());
        assert!(st.get_content("http://a/1", "http://a/c0").expect("get").is_none());
    }
}
