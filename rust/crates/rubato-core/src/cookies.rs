//! `CookieStore.cookieToMap/mapToCookie` 与 `CookieManager.mergeCookies` 的
//! 纯函数面(net 的 setCookie 与 store 的存取共用)。
//! 逐行对照 judge/engine 的 help/http/CookieStore.kt / CookieManager.kt。

/// LinkedHashMap 语义的有序 map:put 覆盖值但保位置
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OrderedMap(pub Vec<(String, String)>);

impl OrderedMap {
    pub fn put(&mut self, key: &str, value: &str) {
        match self.0.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = value.to_string(),
            None => self.0.push((key.to_string(), value.to_string())),
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn remove(&mut self, key: &str) {
        self.0.retain(|(k, _)| k != key);
    }

    pub fn put_all(&mut self, other: &OrderedMap) {
        for (k, v) in &other.0 {
            self.put(k, v);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Kotlin `trim { it <= ' ' }`
fn trim_java(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

/// Kotlin `isNotBlank`(Char.isWhitespace)
fn is_not_blank(s: &str) -> bool {
    s.chars().any(|c| !c.is_whitespace())
}

/// `CookieStore.cookieToMap`
pub fn cookie_to_map(cookie: &str) -> OrderedMap {
    let mut map = OrderedMap::default();
    // isBlank
    if !is_not_blank(cookie) {
        return map;
    }
    // split(semicolonRegex).dropLastWhile { it.isEmpty() }
    let mut pairs: Vec<&str> = cookie.split(';').collect();
    while pairs.last().is_some_and(|p| p.is_empty()) {
        pairs.pop();
    }
    for pair in pairs {
        // split(equalsRegex, 2).dropLastWhile { it.isEmpty() }
        let mut parts: Vec<&str> = pair.splitn(2, '=').collect();
        while parts.last().is_some_and(|p| p.is_empty()) {
            parts.pop();
        }
        if parts.len() <= 1 {
            continue;
        }
        let key = trim_java(parts[0]);
        let value = parts[1];
        if is_not_blank(value) || trim_java(value) == "null" {
            map.put(key, trim_java(value));
        }
    }
    map
}

/// `CookieStore.mapToCookie`:空 map → None
pub fn map_to_cookie(map: &OrderedMap) -> Option<String> {
    if map.is_empty() {
        return None;
    }
    Some(map.0.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; "))
}

/// `CookieManager.mergeCookiesToMap`:右侧覆盖左侧(值更新、位置保留)
pub fn merge_cookies_to_map(cookies: &[Option<&str>]) -> OrderedMap {
    let mut acc = OrderedMap::default();
    for c in cookies.iter().flatten() {
        acc.put_all(&cookie_to_map(c));
    }
    acc
}

/// `CookieManager.mergeCookies`
pub fn merge_cookies(cookies: &[Option<&str>]) -> Option<String> {
    map_to_cookie(&merge_cookies_to_map(cookies))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_roundtrip() {
        let m = cookie_to_map("a=1; b=2;c=3");
        assert_eq!(map_to_cookie(&m).as_deref(), Some("a=1; b=2; c=3"));
    }

    #[test]
    fn blank_value_dropped_unless_null() {
        let m = cookie_to_map("a= ; b=null; c=x");
        assert_eq!(map_to_cookie(&m).as_deref(), Some("b=null; c=x"));
    }

    #[test]
    fn merge_overrides_in_place() {
        let merged = merge_cookies(&[Some("a=1; b=2"), Some("b=9; c=3")]);
        assert_eq!(merged.as_deref(), Some("a=1; b=9; c=3"));
    }

    #[test]
    fn trailing_equals_skipped() {
        let m = cookie_to_map("k=; a=1");
        assert_eq!(map_to_cookie(&m).as_deref(), Some("a=1"));
    }
}
