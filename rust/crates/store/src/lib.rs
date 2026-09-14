//! rusqlite 存储(表名/列名对齐裁判 Room schema,便于日后导入 legado 备份)。
//!
//! - [`db`] —— 单文件库的建表 DDL(books / chapters / caches / highlights / book_sources)
//! - [`book_store`] —— 书架、目录、正文缓存
//! - [`highlight_store`] —— 正文批注(高亮、下划线、笔记)
//! - [`source_store`] —— 书源库
//! - [`cookie_store`] —— CookieStore/CookieManager 的状态面(fetch 套差分钉住)

/// 上层复用同一个 rusqlite 版本(错误类型跨 crate 传递)
pub use rusqlite;

pub mod book_store;
pub mod cookie_store;
pub mod db;
pub mod highlight_store;
pub mod source_store;

pub use book_store::{BookRow, BookStore};
pub use cookie_store::CookieStore;
pub use highlight_store::{HighlightRow, HighlightStore};
pub use source_store::SourceStore;
