//! `CookieStore` + `CookieManager` 的状态面移植(judge/engine 的
//! help/http/CookieStore.kt / CookieManager.kt,一行未省;webkit 相关为空操作)。
//!
//! 存储对齐 legado schema:表 `cookies (url PRIMARY KEY, cookie)`,键是
//! `NetworkUtils.getSubDomain` 归一后的二级域名。session cookie 与
//! `<domain>_cookie` 镜像走 CacheManager 的内存面(HashMap)。
//!
//! 已知不可差分点(用例回避,fixtures/cases/fetch/README.md 记录):
//! `getCookie` 超 4096 字节时真身**随机**删键,这里删第一个键。

use rubato_core::cookies::{cookie_to_map, map_to_cookie, merge_cookies, merge_cookies_to_map};
use rubato_core::host::{CookieEnv, SetCookie};
use rubato_core::net_utils::get_sub_domain;
use rusqlite::Connection;
use std::collections::HashMap;

pub struct CookieStore {
    conn: Connection,
    /// CacheManager.putMemory/getFromMemory 的等价面
    memory: HashMap<String, String>,
}

impl CookieStore {
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::with_conn(Connection::open_in_memory()?)
    }

    pub fn open(path: &str) -> rusqlite::Result<Self> {
        Self::with_conn(Connection::open(path)?)
    }

    fn with_conn(conn: Connection) -> rusqlite::Result<Self> {
        // busy_timeout/WAL:与 BookStore/SourceStore 共库文件,并发搜索时
        // 这条连接也会撞 SQLITE_BUSY,不设就是写失败(见 db::tune)。
        crate::db::tune(&conn)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS cookies (
                url TEXT NOT NULL PRIMARY KEY,
                cookie TEXT NOT NULL
            );",
        )?;
        Ok(CookieStore { conn, memory: HashMap::new() })
    }

    // ---- CookieStore ----

    /// `setCookie(url, cookie)`
    pub fn set_cookie(&mut self, url: &str, cookie: Option<&str>) {
        let domain = get_sub_domain(url);
        let cookie = cookie.unwrap_or("");
        self.memory.insert(format!("{domain}_cookie"), cookie.to_string());
        // cookieDao.insert 是 REPLACE 语义(Room @Insert(onConflict = REPLACE))。
        // 接口面照 Kotlin 是 fire-and-forget(`()`),但失败不能不出声:
        // 静默丢 cookie 的症状是「登录状态莫名其妙没了」,查不出来。
        if let Err(e) = self.conn.execute(
            "INSERT OR REPLACE INTO cookies (url, cookie) VALUES (?1, ?2)",
            (&domain, cookie),
        ) {
            eprintln!("[store] setCookie({domain}) 落库失败(内存面已写,重启后丢): {e}");
        }
    }

    /// `replaceCookie(url, cookie)`
    pub fn replace_cookie(&mut self, url: &str, cookie: &str) {
        if url.is_empty() || cookie.is_empty() {
            return;
        }
        let old_cookie = self.get_cookie_no_session(url);
        if old_cookie.is_empty() {
            self.set_cookie(url, Some(cookie));
        } else {
            let mut map = cookie_to_map(&old_cookie);
            map.put_all(&cookie_to_map(cookie));
            let new_cookie = map_to_cookie(&map);
            self.set_cookie(url, new_cookie.as_deref());
        }
    }

    /// `getCookie(url)`:库 + session 合并;>4096 裁剪(见模块注释)
    pub fn get_cookie(&mut self, url: &str) -> String {
        let domain = get_sub_domain(url);
        let cookie = self.get_cookie_no_session(url);
        let session_cookie = self.get_session_cookie(&domain);
        let mut cookie_map =
            merge_cookies_to_map(&[Some(cookie.as_str()), session_cookie.as_deref()]);
        let mut ck = map_to_cookie(&cookie_map).unwrap_or_default();
        while ck.len() > 4096 {
            let remove_key = match cookie_map.0.first() {
                Some((k, _)) => k.clone(),
                None => break,
            };
            self.remove_cookie_key(url, &remove_key);
            cookie_map.remove(&remove_key);
            ck = map_to_cookie(&cookie_map).unwrap_or_default();
        }
        ck
    }

    /// `getKey(url, key)`(注意真身把**完整 url** 当 session 键查,通常查不到——原样保留)
    pub fn get_key(&mut self, url: &str, key: &str) -> String {
        let cookie = self.get_cookie(url);
        let session_cookie = self.get_session_cookie(url);
        let map = merge_cookies_to_map(&[Some(cookie.as_str()), session_cookie.as_deref()]);
        map.get(key).unwrap_or("").to_string()
    }

    /// `removeCookie(url)`
    pub fn remove_cookie(&mut self, url: &str) {
        let domain = get_sub_domain(url);
        if let Err(e) = self.conn.execute("DELETE FROM cookies WHERE url = ?1", (&domain,)) {
            eprintln!("[store] removeCookie({domain}) 删库失败(内存面已清): {e}");
        }
        self.memory.remove(&format!("{domain}_cookie"));
        self.memory.remove(&format!("{domain}_session_cookie"));
        // android.webkit.CookieManager.removeCookie:非 Android 面,空操作
    }

    // ---- CookieManager ----

    /// `CookieManager.getSessionCookie(domain)`
    pub fn get_session_cookie(&self, domain: &str) -> Option<String> {
        self.memory.get(&format!("{domain}_session_cookie")).cloned()
    }

    fn update_session_cookie(&mut self, domain: &str, cookies: &str) {
        let session_cookie = self.get_session_cookie(domain);
        match session_cookie.as_deref() {
            None | Some("") => {
                self.memory.insert(format!("{domain}_session_cookie"), cookies.to_string());
            }
            Some(existing) => {
                if let Some(ck) = merge_cookies(&[Some(existing), Some(cookies)]) {
                    self.memory.insert(format!("{domain}_session_cookie"), ck);
                }
            }
        }
    }

    /// `CookieManager.saveCookiesFromHeaders`(cookies 已按 okhttp 解析序)
    pub fn save_cookies(&mut self, url: &str, cookies: &[SetCookie]) {
        let domain = get_sub_domain(url);
        let join = |persistent: bool| {
            cookies
                .iter()
                .filter(|c| c.persistent == persistent)
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; ")
        };
        let session_cookie = join(false);
        self.update_session_cookie(&domain, &session_cookie);
        let cookie_string = join(true);
        self.replace_cookie(&domain, &cookie_string);
    }

    /// `CookieManager.loadRequest`:返回新的 Cookie 头值;None = 不改
    pub fn load_request(&mut self, url: &str, request_cookie: Option<&str>) -> Option<String> {
        let domain = get_sub_domain(url);
        let cookie = self.get_cookie(&domain);
        let new_cookie = merge_cookies(&[request_cookie, Some(cookie.as_str())])?;
        // okhttp headersCheckValue:'\t' 或 0x20..=0x7e 之外 → 异常 → 清 cookie
        if new_cookie.chars().all(|c| c == '\t' || ('\u{20}'..='\u{7e}').contains(&c)) {
            Some(new_cookie)
        } else {
            self.remove_cookie(url);
            None
        }
    }

    /// `CookieManager.removeCookie(url, key)`
    pub fn remove_cookie_key(&mut self, url: &str, key: &str) {
        let domain = get_sub_domain(url);
        if let Some(session) = self.get_session_cookie(&domain) {
            let mut map = cookie_to_map(&session);
            map.remove(key);
            if let Some(ck) = map_to_cookie(&map) {
                self.memory.insert(format!("{domain}_session_cookie"), ck);
            }
        }
        let cookie = self.get_cookie_no_session(url);
        if !cookie.is_empty() {
            let mut map = cookie_to_map(&cookie);
            map.remove(key);
            if let Some(ck) = map_to_cookie(&map) {
                self.set_cookie(url, Some(&ck));
            }
        }
    }

    /// `CookieManager.getCookieNoSession`
    pub fn get_cookie_no_session(&mut self, url: &str) -> String {
        let domain = get_sub_domain(url);
        if let Some(cache) = self.memory.get(&format!("{domain}_cookie")) {
            return cache.clone();
        }
        self.conn
            .query_row("SELECT cookie FROM cookies WHERE url = ?1", (&domain,), |r| {
                r.get::<_, String>(0)
            })
            .unwrap_or_default()
    }

    /// 差分预置面:直接写 session 内存(等价 CacheManager.putMemory)
    pub fn put_session_cookie(&mut self, domain: &str, value: &str) {
        self.memory.insert(format!("{domain}_session_cookie"), value.to_string());
    }

    /// 差分观察面:库里的全部 (domain, cookie) + 内存 session
    pub fn dump(&mut self) -> (Vec<(String, String)>, Vec<(String, String)>) {
        let mut db: Vec<(String, String)> = Vec::new();
        if let Ok(mut stmt) = self.conn.prepare("SELECT url, cookie FROM cookies ORDER BY url") {
            if let Ok(rows) = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))) {
                db.extend(rows.flatten());
            }
        }
        let mut session: Vec<(String, String)> = self
            .memory
            .iter()
            .filter(|(k, _)| k.ends_with("_session_cookie"))
            .map(|(k, v)| (k.trim_end_matches("_session_cookie").to_string(), v.clone()))
            .collect();
        session.sort();
        (db, session)
    }
}

impl CookieEnv for CookieStore {
    fn get_cookie(&mut self, url: &str) -> String {
        CookieStore::get_cookie(self, url)
    }

    fn load_request_cookie(&mut self, url: &str, request_cookie: Option<&str>) -> Option<String> {
        self.load_request(url, request_cookie)
    }

    fn save_response_cookies(&mut self, url: &str, cookies: &[SetCookie]) {
        self.save_cookies(url, cookies);
    }

    fn set_cookie(&mut self, url: &str, cookie: &str) {
        CookieStore::set_cookie(self, url, Some(cookie));
    }

    fn replace_cookie(&mut self, url: &str, cookie: &str) {
        CookieStore::replace_cookie(self, url, cookie);
    }

    fn remove_cookie(&mut self, url: &str) {
        CookieStore::remove_cookie(self, url);
    }

    fn get_cookie_key(&mut self, url: &str, key: &str) -> String {
        self.get_key(url, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subdomain_keyed_roundtrip() {
        let mut s = CookieStore::open_in_memory().expect("打开");
        s.set_cookie("http://www.example.com/a", Some("k=1"));
        assert_eq!(s.get_cookie("http://foo.example.com/b"), "k=1");
    }

    #[test]
    fn session_and_persistent_split() {
        let mut s = CookieStore::open_in_memory().expect("打开");
        s.save_cookies(
            "http://example.com/",
            &[
                SetCookie { name: "sid".into(), value: "s".into(), persistent: false },
                SetCookie { name: "uid".into(), value: "p".into(), persistent: true },
            ],
        );
        assert_eq!(s.get_cookie_no_session("http://example.com/"), "uid=p");
        assert_eq!(s.get_session_cookie("example.com").as_deref(), Some("sid=s"));
        assert_eq!(s.get_cookie("http://example.com/"), "uid=p; sid=s");
    }

    #[test]
    fn replace_merges() {
        let mut s = CookieStore::open_in_memory().expect("打开");
        s.set_cookie("http://example.com/", Some("a=1; b=2"));
        s.replace_cookie("http://example.com/", "b=9; c=3");
        assert_eq!(s.get_cookie_no_session("http://example.com/"), "a=1; b=9; c=3");
    }
}
