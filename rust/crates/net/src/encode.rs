//! `AnalyzeUrl.encodeParams` / `encodeQueryParams` / `appendEncoded`。
//!
//! 三条编码路径,口径都以裁判为准:
//! - query 且有 charset:整串先看 `NetworkUtils.encodedQuery`,已编码就原样,
//!   否则走 hutool `queryEncoder`(RFC3986.UNRESERVED + `!$%&()*+,/:;=?@[\]^`{|}`);
//! - `charset == "escape"`:走 `EncoderUtils.escape`(JS 的老 escape 函数);
//! - 其余:按 `&`/`=` 切段后 `URLEncoder.encode`(空格→`+`)。
//!
//! 注意 Kotlin 的 `for (c in str)` 遍历的是 **UTF-16 码元**,所以增补平面字符
//! 在 `EncoderUtils.escape` 里会被拆成两个孤立代理项,这里照做;
//! 而 query 路径在 LegadoTeam 基准换成了 hutool PercentCodec,代理对会被
//! `OutputStreamWriter` 的 leftoverChar 机制重新拼回**按码点**编码(见
//! `encode_query_params`)。

use crate::UrlError;

const QUERY_HEX_UPPER: &[u8] = b"0123456789ABCDEF";

fn query_safe(c: char) -> bool {
    let code = c as u32;
    code < 128 && (c.is_ascii_alphanumeric() || "_.-~!$%&()*+,/:;=?@[\\]^`{|}".contains(c))
}

/// `NetworkUtils.notNeedEncodingQuery`
fn not_need_encoding_query(c: char) -> bool {
    c.is_ascii_alphanumeric() || "!$&()*+,-./:;=?@[\\]^_`{|}~".contains(c)
}

/// `NetworkUtils.notNeedEncodingForm`
fn not_need_encoding_form(c: char) -> bool {
    c.is_ascii_alphanumeric() || "*-._".contains(c)
}

fn is_digit16(c: char) -> bool {
    c.is_ascii_hexdigit()
}

fn already_encoded(s: &str, ok: fn(char) -> bool) -> bool {
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    while i < cs.len() {
        let c = cs[i];
        if ok(c) {
            i += 1;
            continue;
        }
        if c == '%' && i + 2 < cs.len() {
            let c1 = cs[i + 1];
            let c2 = cs[i + 2];
            i += 2;
            if is_digit16(c1) && is_digit16(c2) {
                i += 1;
                continue;
            }
        }
        return false;
    }
    true
}

pub fn encoded_query(s: &str) -> bool {
    already_encoded(s, not_need_encoding_query)
}

pub fn encoded_form(s: &str) -> bool {
    already_encoded(s, not_need_encoding_form)
}

enum Enc {
    /// `charset == "escape"`
    Escape,
    Charset(&'static encoding_rs::Encoding),
}

fn resolve(charset: &str) -> Result<Enc, UrlError> {
    if charset == "escape" {
        return Ok(Enc::Escape);
    }
    encoding_rs::Encoding::for_label(charset.as_bytes())
        .map(Enc::Charset)
        .ok_or_else(|| UrlError::UnsupportedCharset(charset.to_string()))
}

/// 把一个码点编成目标字符集的字节;编不出来按 Java CharsetEncoder 的替换字节 `?`
fn char_bytes(c: char, enc: &'static encoding_rs::Encoding) -> Vec<u8> {
    let s = c.to_string();
    let (bytes, _, had_err) = enc.encode(&s);
    if had_err { b"?".to_vec() } else { bytes.into_owned() }
}

/// `AnalyzeUrl.encodeQueryParams` —— LegadoTeam 基准改走 hutool
/// `RFC3986.UNRESERVED.orNew(PercentCodec.of(...))`(安全字符集与旧实现一致)。
///
/// 代理对语义:hutool 的 PercentCodec 逐 UTF-16 码元喂 `OutputStreamWriter`
/// 再 flush,而 `StreamEncoder` 把高代理项留在 leftoverChar 里(flush 不吐),
/// 下一个低代理项到齐后才整体编码 —— 净效果就是**按码点编码**:
/// `😀` 在 UTF-8 下出 `%F0%9F%98%80`、gb18030 下出 `%94%39%FC%36`、
/// GBK/big5/latin1 下出单个 `%3F`。旧基准逐码元编码,出两个 `%3F`,
/// 这是本次基准切换的语义漂移点之一。
fn encode_query_params(s: &str, enc: &'static encoding_rs::Encoding) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if query_safe(c) {
            out.push(c);
            continue;
        }
        for b in char_bytes(c, enc) {
            out.push('%');
            out.push(QUERY_HEX_UPPER[(b >> 4) as usize] as char);
            out.push(QUERY_HEX_UPPER[(b & 0x0F) as usize] as char);
        }
    }
    out
}

/// `EncoderUtils.escape`(按 UTF-16 码元)
fn escape(s: &str) -> String {
    let mut out = String::new();
    for unit in s.encode_utf16() {
        let code = unit as u32;
        if (48..=57).contains(&code) || (65..=90).contains(&code) || (97..=122).contains(&code) {
            out.push(char::from_u32(code).expect("ASCII"));
            continue;
        }
        let prefix = if code < 16 {
            "%0"
        } else if code < 256 {
            "%"
        } else {
            "%u"
        };
        out.push_str(prefix);
        out.push_str(&format!("{code:x}"));
    }
    out
}

/// `java.net.URLEncoder.encode(s, charset)`(js-host 的 `java.encodeURI` 也走它)
pub fn url_encoder_encode(s: &str, enc: &'static encoding_rs::Encoding) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '*' | '_') {
            out.push(c);
        } else if c == ' ' {
            out.push('+');
        } else {
            let cs = c.to_string();
            let (bytes, _, had_err) = enc.encode(&cs);
            let bytes: Vec<u8> = if had_err { b"?".to_vec() } else { bytes.into_owned() };
            for b in bytes {
                out.push('%');
                out.push(QUERY_HEX_UPPER[(b >> 4) as usize] as char);
                out.push(QUERY_HEX_UPPER[(b & 0x0F) as usize] as char);
            }
        }
    }
    out
}

fn append_encoded(out: &mut String, value: &str, check_encoded: bool, enc: &Enc) {
    if check_encoded && encoded_form(value) {
        out.push_str(value);
    } else {
        match enc {
            Enc::Escape => out.push_str(&escape(value)),
            Enc::Charset(c) => out.push_str(&url_encoder_encode(value, c)),
        }
    }
}

/// `AnalyzeUrl.encodeParams`
pub fn encode_params(
    params: &str,
    charset: Option<&str>,
    is_query: bool,
) -> Result<String, UrlError> {
    let check_encoded = charset.is_none_or(str::is_empty);
    let enc = match charset {
        None | Some("") => Enc::Charset(encoding_rs::UTF_8),
        Some(c) => resolve(c)?,
    };
    if is_query {
        if let Enc::Charset(c) = enc {
            if encoded_query(params) {
                return Ok(params.to_string());
            }
            return Ok(encode_query_params(params, c));
        }
    }
    let cs: Vec<char> = params.chars().collect();
    let len = cs.len();
    let mut sb = String::new();
    let mut pos = 0usize;
    while pos <= len {
        if !sb.is_empty() {
            sb.push('&');
        }
        let amp_offset = find_from(&cs, '&', pos).unwrap_or(len);
        let eq_offset = find_from(&cs, '=', pos);
        let (key, value): (String, Option<String>) = match eq_offset {
            Some(eq) if eq <= amp_offset => (
                cs[pos.min(len)..eq].iter().collect(),
                Some(cs[eq + 1..amp_offset].iter().collect()),
            ),
            _ => (cs[pos.min(len)..amp_offset].iter().collect(), None),
        };
        append_encoded(&mut sb, &key, check_encoded, &enc);
        if let Some(value) = value {
            sb.push('=');
            append_encoded(&mut sb, &value, check_encoded, &enc);
        }
        pos = amp_offset + 1;
    }
    Ok(sb)
}

fn find_from(cs: &[char], needle: char, from: usize) -> Option<usize> {
    (from.min(cs.len())..cs.len()).find(|&i| cs[i] == needle)
}
