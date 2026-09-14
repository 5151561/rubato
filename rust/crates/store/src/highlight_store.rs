//! 正文批注(`highlights`)的落库面:高亮、下划线,以及挂在它们上面的笔记。
//!
//! 表结构见 [`crate::db::SCHEMA`]，列名逐字对齐裁判的 `BookHighlight.kt`
//! （Phase 4 要导入 legado 备份的 `highlight.json`，差一列就得再写一套映射）。
//!
//! # 位置口径
//!
//! `chapterPos` / `chapterPosEnd` 是**章内 UTF-16 code unit 下标**，与
//! `books.durChapterPos` 同一把尺（计划书 §3.1 / §5.5）。原点含章节标题那一块
//! —— 上游同款，它另存一列 `layoutTitleLength` 好把标题那一截减回去。
//! Rubato 这一侧同样写这一列：不写的话，一份 Rubato 的备份导进裁判就会整体错位
//! 一个标题的长度，而那种错位**看不出来是错位**，只像「批注飘了一点」。
//!
//! # `style`
//!
//! 上游那份是一个能组合的样式对象（填色、字色、粗斜、下划线、删除线、外框、
//! 着重号、阴影、字体路径）。Rubato 只画其中两件：`fill`（一层底色）与
//! `underline`（一道线）。**存的仍是上游那套 JSON 的字段名与形状** —— 认不出的
//! 字段原样留着，导出去还是它；日后要画删除线，是多认一个字段，不是改表。

use rusqlite::{Connection, Row, params};

/// 一条批注。`time` 既是主键也是创建时间（上游同款）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HighlightRow {
    pub time: i64,
    pub book_url: String,
    pub chapter_url: String,
    pub book_name: String,
    pub book_author: String,
    pub chapter_index: i32,
    /// 章内 UTF-16 下标，闭开区间 `[chapter_pos, chapter_pos_end)`
    pub chapter_pos: i32,
    pub chapter_pos_end: i32,
    /// 这一章的标题占了多少个 code unit。`-1` = 不知道（上游默认值）
    pub layout_title_length: i32,
    pub chapter_name: String,
    /// 划下的那段正文原样。**冗余是有意的**：换了书源、正文改了版之后，
    /// 下标可能已经指不回原处，那时这一段是唯一能告诉读者「你当初划的是什么」的东西
    pub book_text: String,
    /// 上游那套样式 JSON 原样（见模块注释）
    pub style: String,
    pub note: String,
}

pub struct HighlightStore;

impl HighlightStore {
    /// 一章里的全部批注，按位置排。**按 (bookUrl, chapterIndex) 取** ——
    /// 章的 URL 会随书源换而变，章号不会
    pub fn list(
        conn: &Connection,
        book_url: &str,
        chapter_index: i32,
    ) -> rusqlite::Result<Vec<HighlightRow>> {
        let mut st = conn.prepare(
            "SELECT time, bookUrl, chapterUrl, bookName, bookAuthor, chapterIndex,
                 chapterPos, chapterPosEnd, layoutTitleLength, chapterName, bookText, style, note
             FROM highlights WHERE bookUrl = ?1 AND chapterIndex = ?2
             ORDER BY chapterPos, chapterPosEnd, time",
        )?;
        let rows = st.query_map(params![book_url, chapter_index], read_row)?;
        rows.collect()
    }

    /// 一本书的全部批注，按章号再按位置排（批注列表那一屏用它）
    pub fn list_book(conn: &Connection, book_url: &str) -> rusqlite::Result<Vec<HighlightRow>> {
        let mut st = conn.prepare(
            "SELECT time, bookUrl, chapterUrl, bookName, bookAuthor, chapterIndex,
                 chapterPos, chapterPosEnd, layoutTitleLength, chapterName, bookText, style, note
             FROM highlights WHERE bookUrl = ?1
             ORDER BY chapterIndex, chapterPos, time",
        )?;
        let rows = st.query_map(params![book_url], read_row)?;
        rows.collect()
    }

    /// 新增或整条覆盖。`time` 是主键 —— 调用方给 0 时按当前时间取一个
    /// **不与既有行相撞**的：同一毫秒内连划两段是完全可能的（判据就这么干）
    pub fn save(conn: &Connection, row: &HighlightRow, now_ms: i64) -> rusqlite::Result<i64> {
        let mut time = if row.time > 0 { row.time } else { now_ms };
        if row.time <= 0 {
            while conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM highlights WHERE time = ?1)",
                params![time],
                |r| r.get::<_, bool>(0),
            )? {
                time += 1;
            }
        }
        conn.execute(
            "INSERT OR REPLACE INTO highlights (time, bookUrl, chapterUrl, bookName, bookAuthor,
                 chapterIndex, chapterPos, chapterPosEnd, layoutTitleLength, chapterName,
                 bookText, style, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                time,
                row.book_url,
                row.chapter_url,
                row.book_name,
                row.book_author,
                row.chapter_index,
                row.chapter_pos,
                row.chapter_pos_end,
                row.layout_title_length,
                row.chapter_name,
                row.book_text,
                row.style,
                row.note,
            ],
        )?;
        Ok(time)
    }

    /// 删一条。返回「真删掉了吗」
    pub fn delete(conn: &Connection, time: i64) -> rusqlite::Result<bool> {
        Ok(conn.execute("DELETE FROM highlights WHERE time = ?1", params![time])? > 0)
    }

    /// 换一条的样式或笔记。**不动位置** —— 位置是划的时候定的
    pub fn update(conn: &Connection, time: i64, style: &str, note: &str) -> rusqlite::Result<bool> {
        Ok(conn.execute(
            "UPDATE highlights SET style = ?2, note = ?3 WHERE time = ?1",
            params![time, style, note],
        )? > 0)
    }

    /// 一本书被移出书架时跟着走。批注挂在书上，留着就是一堆再也打不开的孤儿
    pub fn delete_book(conn: &Connection, book_url: &str) -> rusqlite::Result<usize> {
        conn.execute("DELETE FROM highlights WHERE bookUrl = ?1", params![book_url])
    }
}

fn read_row(r: &Row<'_>) -> rusqlite::Result<HighlightRow> {
    Ok(HighlightRow {
        time: r.get(0)?,
        book_url: r.get(1)?,
        chapter_url: r.get(2)?,
        book_name: r.get(3)?,
        book_author: r.get(4)?,
        chapter_index: r.get(5)?,
        chapter_pos: r.get(6)?,
        chapter_pos_end: r.get(7)?,
        layout_title_length: r.get(8)?,
        chapter_name: r.get(9)?,
        book_text: r.get(10)?,
        style: r.get(11)?,
        note: r.get(12)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn row(pos: i32, end: i32) -> HighlightRow {
        HighlightRow {
            book_url: "http://a/1".into(),
            chapter_index: 3,
            chapter_pos: pos,
            chapter_pos_end: end,
            layout_title_length: 5,
            book_text: "溪畔".into(),
            style: r#"{"fill":-2147418113}"#.into(),
            ..Default::default()
        }
    }

    #[test]
    fn 按章取回来并按位置排序() {
        let conn = db::open(":memory:").expect("open");
        HighlightStore::save(&conn, &row(90, 100), 1000).expect("save");
        HighlightStore::save(&conn, &row(10, 20), 1001).expect("save");
        let got = HighlightStore::list(&conn, "http://a/1", 3).expect("list");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].chapter_pos, 10);
        assert_eq!(got[1].chapter_pos, 90);
        // 另一章、另一本都取不到
        assert!(HighlightStore::list(&conn, "http://a/1", 4).expect("list").is_empty());
        assert!(HighlightStore::list(&conn, "http://b/1", 3).expect("list").is_empty());
    }

    #[test]
    fn 同一毫秒里连划两段不会互相顶掉() {
        let conn = db::open(":memory:").expect("open");
        let a = HighlightStore::save(&conn, &row(10, 20), 7).expect("save");
        let b = HighlightStore::save(&conn, &row(30, 40), 7).expect("save");
        assert_ne!(a, b, "两条批注拿到了同一个主键");
        assert_eq!(HighlightStore::list(&conn, "http://a/1", 3).expect("list").len(), 2);
    }

    #[test]
    fn 改样式不动位置_删得掉() {
        let conn = db::open(":memory:").expect("open");
        let id = HighlightStore::save(&conn, &row(10, 20), 7).expect("save");
        assert!(HighlightStore::update(&conn, id, r#"{"fill":1}"#, "记一笔").expect("update"));
        let got = HighlightStore::list(&conn, "http://a/1", 3).expect("list");
        assert_eq!(got[0].style, r#"{"fill":1}"#);
        assert_eq!(got[0].note, "记一笔");
        assert_eq!((got[0].chapter_pos, got[0].chapter_pos_end), (10, 20));
        assert_eq!(got[0].layout_title_length, 5, "标题长度是导出给裁判用的,不许丢");

        assert!(HighlightStore::delete(&conn, id).expect("delete"));
        assert!(!HighlightStore::delete(&conn, id).expect("delete"), "删两遍第二遍该是 false");
        assert!(HighlightStore::list(&conn, "http://a/1", 3).expect("list").is_empty());
    }

    #[test]
    fn 整本删干净() {
        let conn = db::open(":memory:").expect("open");
        HighlightStore::save(&conn, &row(10, 20), 7).expect("save");
        let mut other = row(10, 20);
        other.book_url = "http://b/1".into();
        HighlightStore::save(&conn, &other, 8).expect("save");
        assert_eq!(HighlightStore::delete_book(&conn, "http://a/1").expect("del"), 1);
        assert!(HighlightStore::list_book(&conn, "http://a/1").expect("list").is_empty());
        assert_eq!(HighlightStore::list_book(&conn, "http://b/1").expect("list").len(), 1);
    }
}
