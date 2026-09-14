//! 书源库(`book_sources`)。**原始 JSON 整串落库**,SQL 只碰排序/过滤要用的
//! 三列(bookSourceUrl/enabled/customOrder);读回时交 [`BookSource::from_value`]
//! —— 解析口径与导入书源 JSON 时**同一条路径**(source 套 1727 例差分钉住的
//! 那条),且「解析时丢掉的字段」不会在落库往返中被永久抹掉。

use crate::db;
use rubato_core::entities::BookSource;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::Value;

pub struct SourceStore {
    conn: Connection,
}

impl SourceStore {
    pub fn open(path: &str) -> rusqlite::Result<SourceStore> {
        Ok(SourceStore { conn: db::open(path)? })
    }

    pub fn open_in_memory() -> rusqlite::Result<SourceStore> {
        SourceStore::open(":memory:")
    }

    /// 导入一批书源(`bookSourceDao.insert`,REPLACE 语义)。
    /// `raw` 是该源的原始 JSON 值,整串落库;实体只用来验形与取三列。
    pub fn save_sources(&mut self, raw: &[Value]) -> rusqlite::Result<usize> {
        let tx = self.conn.transaction()?;
        let mut n = 0usize;
        {
            let mut st = tx.prepare(
                "INSERT OR REPLACE INTO book_sources (bookSourceUrl, enabled, customOrder, json)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for v in raw {
                let Ok(s) = BookSource::from_value(v) else { continue };
                if s.book_source_url.is_empty() {
                    continue;
                }
                st.execute(params![s.book_source_url, s.enabled, s.custom_order, v.to_string()])?;
                n += 1;
            }
        }
        tx.commit()?;
        Ok(n)
    }

    pub fn get_source(&self, url: &str) -> rusqlite::Result<Option<BookSource>> {
        self.conn
            .query_row(&format!("{SELECT} WHERE bookSourceUrl = ?1"), params![url], source_row)
            .optional()
            .map(Option::flatten)
    }

    /// 启用中的书源,按 `customOrder` 排(对齐 `bookSourceDao.allEnabled`)
    pub fn list_enabled(&self) -> rusqlite::Result<Vec<BookSource>> {
        let mut st =
            self.conn.prepare(&format!("{SELECT} WHERE enabled = 1 ORDER BY customOrder ASC"))?;
        let rows = st.query_map([], source_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?.into_iter().flatten().collect())
    }

    pub fn list_all(&self) -> rusqlite::Result<Vec<BookSource>> {
        let mut st = self.conn.prepare(&format!("{SELECT} ORDER BY customOrder ASC"))?;
        let rows = st.query_map([], source_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?.into_iter().flatten().collect())
    }

    pub fn count(&self) -> rusqlite::Result<i64> {
        self.conn.query_row("SELECT COUNT(*) FROM book_sources", [], |r| r.get(0))
    }

    pub fn set_enabled(&mut self, url: &str, enabled: bool) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE book_sources SET enabled = ?2 WHERE bookSourceUrl = ?1",
            params![url, enabled],
        )?;
        Ok(())
    }

    pub fn delete_source(&mut self, url: &str) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM book_sources WHERE bookSourceUrl = ?1", params![url])?;
        Ok(())
    }
}

const SELECT: &str = "SELECT enabled, customOrder, json FROM book_sources";

/// `json` 列 → 实体(与导入同一条解析路径)。enabled / customOrder 以**列**
/// 为准,解析前先回填 JSON——实体字段与交给 JS 的 `raw` 必须是
/// 同一个值,`set_enabled` 之后不能让 `source.enabled` 仍读到导入时的旧值。
fn source_row(r: &Row<'_>) -> rusqlite::Result<Option<BookSource>> {
    let enabled: bool = r.get(0)?;
    let custom_order: i32 = r.get(1)?;
    let json: String = r.get(2)?;
    let Ok(mut raw) = serde_json::from_str::<Value>(&json) else { return Ok(None) };
    let Some(m) = raw.as_object_mut() else { return Ok(None) };
    m.insert("enabled".into(), Value::Bool(enabled));
    m.insert("customOrder".into(), Value::from(custom_order));
    Ok(BookSource::from_value(&raw).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"{
        "bookSourceUrl": "http://a.com",
        "bookSourceName": "甲源",
        "bookSourceType": 0,
        "customOrder": 2,
        "enabled": true,
        "searchUrl": "http://a.com/s?k={{key}}",
        "ruleSearch": {"bookList": "@css:li", "name": "@css:h1@text"},
        "ruleToc": "{\"chapterList\":\"@css:li\"}",
        "header": {"User-Agent": "x"}
    }"#;

    #[test]
    fn round_trip_keeps_rules() {
        let mut st = SourceStore::open_in_memory().expect("open");
        let v: Value = serde_json::from_str(SRC).expect("json");
        assert_eq!(st.save_sources(&[v]).expect("save"), 1);
        let s = st.get_source("http://a.com").expect("get").expect("有");
        assert_eq!(s.book_source_name, "甲源");
        assert_eq!(s.custom_order, 2);
        assert_eq!(s.search_rule().book_list.as_deref(), Some("@css:li"));
        assert_eq!(s.search_rule().name.as_deref(), Some("@css:h1@text"));
        // 规则组写成 JSON 串的写法照样回得来
        assert_eq!(s.toc_rule().chapter_list.as_deref(), Some("@css:li"));
        // header 是对象 → 实体里是紧凑 JSON 串
        assert_eq!(s.header.as_deref(), Some(r#"{"User-Agent":"x"}"#));
    }

    #[test]
    fn enabled_filter_and_replace() {
        let mut st = SourceStore::open_in_memory().expect("open");
        let v: Value = serde_json::from_str(SRC).expect("json");
        st.save_sources(std::slice::from_ref(&v)).expect("save");
        // 同 url 再导入是替换而非新增
        st.save_sources(&[v]).expect("save");
        assert_eq!(st.count().expect("count"), 1);
        assert_eq!(st.list_enabled().expect("list").len(), 1);
        st.set_enabled("http://a.com", false).expect("disable");
        assert!(st.list_enabled().expect("list").is_empty());
        st.conn
            .execute(
                "UPDATE book_sources SET customOrder = 9 WHERE bookSourceUrl = ?1",
                ["http://a.com"],
            )
            .expect("reorder");
        let all = st.list_all().expect("all");
        assert_eq!(all.len(), 1);
        assert!(!all[0].enabled);
        assert_eq!(all[0].custom_order, 9);
        let raw = all[0].raw.as_ref().expect("raw");
        assert_eq!(raw["enabled"], false);
        assert_eq!(raw["customOrder"], 9);
    }
}
