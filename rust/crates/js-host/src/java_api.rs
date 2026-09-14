//! `JsExtensions` / `JsEncodeUtils` 的移植:JS 里 `java.*` 能调到的宿主 API。
//!
//! 口径由 **js-host 差分套**钉(裁判是 `judge/jsharness` 上的真身
//! `JsExtensions`,契约见 `fixtures/cases/js-host/README.md`)。
//! 覆盖顺序按语料频次走(B 层 622 源里:`ajax` 116 / `put` 92 / `getString` 87 /
//! `get` 76 / `timeFormat` 68 / `log` 36 / `md5Encode` 31 /
//! `aesBase64DecodeToString` 19 / …),不做「先把 1332 行抄完」。
//!
//! **重载分发**:Rhino 按实参的运行时类型挑 Java 重载,JS 侧没有类型,
//! 所以同名多态在这里按 **arity + 实参 JS 类型** 手工分发
//! (典型:`base64Decode(str, "UTF-8")` 走 charset 重载,
//! `base64Decode(str, 2)` 走 flags 重载)。

use base64::Engine as _;
use chrono::{Local, TimeZone, Utc};

/// android.util.Base64 的 flag 位(AOSP 常量,裁判侧垫片逐位对齐)
pub const B64_DEFAULT: i32 = 0;
pub const B64_NO_PADDING: i32 = 1;
pub const B64_NO_WRAP: i32 = 2;
pub const B64_CRLF: i32 = 4;
pub const B64_URL_SAFE: i32 = 8;

/// 宿主的环境常量:产品侧由 engine 注入,差分侧用裁判垫片里的固定值
/// (见 fixtures/cases/js-host/README.md「不可差分面」)。
#[derive(Debug, Clone, Default)]
pub struct HostConfig {
    /// `java.androidId()`(真身读 `Settings.Secure.ANDROID_ID`)
    pub android_id: String,
    /// `java.getWebViewUA()`(真身读 `WebSettings.getDefaultUserAgent`)
    pub web_view_ua: String,
    /// 随机字节从哪来。**缺省是真随机** —— 定死的那一支只有差分侧能选,
    /// 见 [`RandomSource`]。
    pub random: RandomSource,
}

/// `java.randomUUID()` 这类调用的随机字节来源。
///
/// 这一位曾经不存在:`randomUUID` 无条件走定死的那条流,于是**产品里**每次跑、
/// 每个用户拿到的都是同一串 UUID。书源拿它当设备号 / 会话号拼签名时,轻则请求
/// 被拒,重则所有用户在服务端是同一个指纹。差分要的确定性和产品要的随机性是
/// 两件事,所以做成开关。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RandomSource {
    /// **产品侧**:操作系统熵源(真身 `UUID.randomUUID()` 底下是 `SecureRandom`)
    #[default]
    System,
    /// **差分侧**:与裁判 `jsharness/DeterministicRandom` 逐字对齐的定死流,
    /// 见 [`deterministic_random_bytes_at`]。
    Difftest,
}

impl RandomSource {
    /// 取一个 32 字节块。`counter` 是**本次求值内**的调用序号
    /// (裁判每 case `reset()`,所以从 0 起);`System` 忽略它。
    ///
    /// 拿不到系统熵源是环境级故障(真身那里 SecureRandom 也会抛)——但这条
    /// 路径在 JS 宿主回调里,panic 会穿到 worker 线程再被 join 吞成「0 个源」,
    /// 所以以 `Err` 交给调用方抛成 JS 异常,**不 panic 也不静默退回可预测的值**。
    pub fn bytes_at(self, counter: u64) -> Result<[u8; 32], String> {
        match self {
            RandomSource::Difftest => Ok(deterministic_random_bytes_at(counter)),
            RandomSource::System => {
                let mut b = [0u8; 32];
                getrandom::fill(&mut b).map_err(|e| format!("系统熵源不可用: {e}"))?;
                Ok(b)
            }
        }
    }
}

// ---------- 时间 ----------

/// `java.timeFormat(time)`:`AppConst.dateFormat` = `yyyy/MM/dd HH:mm`,**默认时区**
pub fn time_format(millis: i64) -> String {
    match Local.timestamp_millis_opt(millis).single() {
        Some(dt) => format_java_pattern("yyyy/MM/dd HH:mm", &dt.naive_local()),
        // Java 的 Date 不会「无对应本地时间」,这里只是 chrono 的映射兜底
        None => String::new(),
    }
}

/// `java.timeFormatUTC(time, format, sh)`:`SimpleTimeZone(sh, "UTC")` —— 注意
/// **sh 的单位是毫秒**(SimpleTimeZone 的 rawOffset),不是小时。
pub fn time_format_utc(millis: i64, pattern: &str, raw_offset_ms: i32) -> String {
    let shifted = millis + raw_offset_ms as i64;
    match Utc.timestamp_millis_opt(shifted).single() {
        Some(dt) => format_java_pattern(pattern, &dt.naive_utc()),
        None => String::new(),
    }
}

/// `java.text.SimpleDateFormat` 的**子集**:书源里真实用到的字母
/// (y/M/d/H/h/m/s/S/E/a)+ 单引号字面量。不支持的字母原样输出。
fn format_java_pattern(pattern: &str, dt: &chrono::NaiveDateTime) -> String {
    use chrono::{Datelike, Timelike};
    let mut out = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // 单引号:字面量('' 表示一个引号)
        if c == '\'' {
            i += 1;
            if i < chars.len() && chars[i] == '\'' {
                out.push('\'');
                i += 1;
                continue;
            }
            while i < chars.len() && chars[i] != '\'' {
                out.push(chars[i]);
                i += 1;
            }
            i += 1;
            continue;
        }
        if !c.is_ascii_alphabetic() {
            out.push(c);
            i += 1;
            continue;
        }
        let mut n = 0;
        while i + n < chars.len() && chars[i + n] == c {
            n += 1;
        }
        i += n;
        let pad = |v: i64, w: usize| format!("{:0w$}", v, w = w);
        match c {
            'y' => {
                let y = dt.year() as i64;
                out.push_str(&if n == 2 { pad(y % 100, 2) } else { pad(y, n) });
            }
            'M' => {
                let m = dt.month() as i64;
                if n >= 3 {
                    // 英文月名(Locale.getDefault();差分机器是英文 locale)
                    #[rustfmt::skip] // 手排的分列表:rustfmt 会拆成一行一个
                    const NAMES: [&str; 12] = [
                        "January", "February", "March", "April", "May", "June", "July", "August",
                        "September", "October", "November", "December",
                    ];
                    let full = NAMES[(m - 1) as usize];
                    out.push_str(if n == 3 { &full[..3] } else { full });
                } else {
                    out.push_str(&pad(m, n));
                }
            }
            'd' => out.push_str(&pad(dt.day() as i64, n)),
            'H' => out.push_str(&pad(dt.hour() as i64, n)),
            'h' => {
                let h = dt.hour() % 12;
                out.push_str(&pad(if h == 0 { 12 } else { h as i64 }, n));
            }
            'm' => out.push_str(&pad(dt.minute() as i64, n)),
            's' => out.push_str(&pad(dt.second() as i64, n)),
            'S' => out.push_str(&pad((dt.and_utc().timestamp_subsec_millis()) as i64, n)),
            'a' => out.push_str(if dt.hour() < 12 { "AM" } else { "PM" }),
            'E' => {
                const DAYS: [&str; 7] =
                    ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
                let d = DAYS[dt.weekday().num_days_from_monday() as usize];
                out.push_str(if n >= 4 { d } else { &d[..3] });
            }
            _ => {
                for _ in 0..n {
                    out.push(c);
                }
            }
        }
    }
    out
}

// ---------- 编码 ----------

/// `java.encodeURI(str[, enc])` = `java.net.URLEncoder.encode`;
/// 字符集不认识时真身抛 `UnsupportedEncodingException` 被吞成空串。
pub fn encode_uri(s: &str, enc: Option<&str>) -> String {
    let charset = match enc {
        None => encoding_rs::UTF_8,
        Some(name) => match encoding_rs::Encoding::for_label(name.as_bytes()) {
            Some(e) => e,
            None => return String::new(),
        },
    };
    net::encode::url_encoder_encode(s, charset)
}

/// `HexUtil.encodeHexStr(utf8)`(小写)
pub fn hex_encode(s: &str) -> String {
    hex::encode(s.as_bytes())
}

/// `HexUtil.decodeHexStr(hex)`:hutool 对非法输入抛,真身不吞 → 这里回 Err。
/// 类别是 **UtilException**(hutool 把 `HexUtil` 里的一切都包成它),而且
/// **标签要带书名号** —— `«host:…»` 才是差分侧认的那一份(js_case_runner.rs
/// 按 `«host:` 切);此前写成裸的 `host:HexException`,两处都错,于是这条
/// 落到了 `js:throw` 那一档(js-corpus-ef85223dda)。
pub fn hex_decode_to_string(s: &str) -> Result<String, String> {
    let bytes = hex::decode(s).map_err(|_| "«host:UtilException»".to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// `android.util.Base64.encodeToString(bytes, flags)`
pub fn base64_encode_bytes(bytes: &[u8], flags: i32) -> String {
    let body = if flags & B64_URL_SAFE != 0 {
        if flags & B64_NO_PADDING != 0 {
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
        } else {
            base64::engine::general_purpose::URL_SAFE.encode(bytes)
        }
    } else if flags & B64_NO_PADDING != 0 {
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(bytes)
    } else {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    };
    if flags & B64_NO_WRAP != 0 {
        return body;
    }
    // AOSP 默认每 76 字符断行,**末尾也带一个换行**
    let nl = if flags & B64_CRLF != 0 { "\r\n" } else { "\n" };
    let mut out = String::with_capacity(body.len() + body.len() / 76 * 2 + 2);
    let b = body.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let end = (i + 76).min(b.len());
        out.push_str(&body[i..end]);
        out.push_str(nl);
        i = end;
    }
    out
}

/// `android.util.Base64.decode(str|bytes, flags)`。
///
/// **口径来自裁判侧的垫片**(`jsharness/shims/AndroidUtilShims.kt`):Android 的类
/// JVM 上没有,裁判用 `java.util.Base64.getMimeDecoder()` 复刻 AOSP 的解码器 ——
/// 于是「表外字符直接跳过」(MIME 解码器的语义),而不是像 AOSP 那样抛。
/// 清洗顺序也照它:去 `\r\n` → `-_` 换成 `+/` → 按**清洗后长度**补 `=`。
///
/// 抛 `IllegalArgumentException` 只剩一种情形:末组只剩 1 个字符(4n+1),
/// 那是 base64 里不可能出现的长度。
pub fn base64_decode_bytes(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s
        .chars()
        .filter(|c| !matches!(c, '\r' | '\n'))
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    // 按**字符数**取模,不是字节数:垫片那边是 Kotlin 的 String.length,
    // 而这里的串是按 ISO-8859-1 从 byte[] 造的(0x80..0xFF 在 Rust 里占 2 字节)
    let padded = match cleaned.chars().count() % 4 {
        2 => format!("{cleaned}=="),
        3 => format!("{cleaned}="),
        _ => cleaned,
    };
    // 逐字符走 JDK `Base64.Decoder.decode0` 的那台状态机(isMIME 分支):
    // 表外字符 `continue`;`=` 的**位置**要合法 —— 这一条是错误类别的分水岭,
    // 实测 `js-corpus-c9d744b7c7`(把一段中文喂给 Base64.decode)靠它才对上:
    // 中文的字节全在表外被跳过,剩下的有效字符恰好凑满 4 的倍数,随后垫片补的
    // `==` 落在「一个新组的开头」→ IllegalArgumentException。
    let iae = || "«host:IllegalArgumentException»".to_string();
    let mut kept = String::with_capacity(padded.len());
    let mut unit = 0usize; // 当前四字符组里已收的有效字符数(0..3)
    let mut it = padded.chars();
    while let Some(c) = it.next() {
        if c == '=' {
            // 组的开头就是 `=`(前面刚好凑满一组)→ 非法
            if unit == 0 {
                return Err(iae());
            }
            // 只收了 2 个字符时,后面必须紧跟第二个 `=`
            if unit == 2 && it.next() != Some('=') {
                return Err(iae());
            }
            break;
        }
        if !(c.is_ascii_alphanumeric() || matches!(c, '+' | '/')) {
            continue; // MIME 解码器:表外字符直接跳过
        }
        kept.push(c);
        unit = (unit + 1) % 4;
    }
    // 末组只剩 1 个字符 → 不够拼出一个字节
    if unit == 1 {
        return Err(iae());
    }
    // 末组多出来的位不校验(JDK 的解码器同样不校验)
    let engine = base64::engine::general_purpose::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::general_purpose::NO_PAD.with_decode_allow_trailing_bits(true),
    );
    engine.decode(kept.as_bytes()).map_err(|_| iae())
}

/// **hutool `Base64.decode` 的宽松解码**:查找表外的字符直接跳过,不报错。
///
/// 与 `android.util.Base64`(严格,坏输入抛 `IllegalArgumentException`)是**两个
/// 解码器**,书源能分辨:`java.base64Decode` 走 hutool,
/// `java.base64DecodeToByteArray` 走 android(`EncoderUtils.base64DecodeToByteArray`)。
/// 混用会把错误类别钉错(实测 `js-api-base64-decode-nopad` / `js-corpus-e1d3254e62`)。
pub fn base64_decode_lenient(s: &str) -> Vec<u8> {
    let filtered: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '-' | '_'))
        .collect();
    // 长度模 4 余 1 在 base64 里不可能出现;hutool 那边最后那一个字符解不出整字节
    let filtered =
        if filtered.len() % 4 == 1 { &filtered[..filtered.len() - 1] } else { &filtered[..] };
    // `decode_allow_trailing_bits`:hutool 只做「移位拼字节」,末组多出来的
    // 那几位它直接丢掉,不校验是不是零。不放开这一条,`YWJj_-` 这种输入
    // base64 crate 会报 InvalidLastSymbol,而真身给的是 `abc\u{FFFD}`
    // (末组两字符 → 1 个字节 0xFF,不是合法 UTF-8)。
    let engine = base64::engine::general_purpose::GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        base64::engine::general_purpose::NO_PAD.with_decode_allow_trailing_bits(true),
    );
    // URL-safe 的 `-_` 与标准的 `+/` 混在一起也要吃下(hutool 的表两套都收)
    let normalized: String = filtered
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    engine.decode(normalized.as_bytes()).unwrap_or_default()
}

/// `java.base64Decode(str[, charset])`:hutool `Base64.decodeStr`(**宽松**)
pub fn base64_decode_str(s: &str, charset: Option<&str>) -> Result<String, String> {
    let bytes = base64_decode_lenient(s);
    let enc = match charset {
        None => encoding_rs::UTF_8,
        Some(name) => encoding_rs::Encoding::for_label(name.as_bytes())
            .ok_or_else(|| "host:UnsupportedCharsetException".to_string())?,
    };
    let (cow, _, _) = enc.decode(&bytes);
    Ok(cow.into_owned())
}

/// `str.toByteArray(charset(name))`(`strToBytes`)。名字不认识时 Java 抛
/// `UnsupportedCharsetException`,两侧按类别比。
pub fn encode_with_charset(s: &str, charset: Option<&str>) -> Result<Vec<u8>, String> {
    let enc = match charset {
        None => encoding_rs::UTF_8,
        Some(name) => encoding_rs::Encoding::for_label(name.as_bytes())
            .ok_or_else(|| "host:UnsupportedCharsetException".to_string())?,
    };
    let (cow, _, _) = enc.encode(s);
    Ok(cow.into_owned())
}

/// `String(bytes, charset(name))`(`bytesToStr`)
pub fn decode_with_charset(bytes: &[u8], charset: Option<&str>) -> Result<String, String> {
    let enc = match charset {
        None => encoding_rs::UTF_8,
        Some(name) => encoding_rs::Encoding::for_label(name.as_bytes())
            .ok_or_else(|| "host:UnsupportedCharsetException".to_string())?,
    };
    let (cow, _, _) = enc.decode(bytes);
    Ok(cow.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_pattern_subset() {
        let dt =
            chrono::NaiveDate::from_ymd_opt(2023, 11, 14).unwrap().and_hms_opt(22, 13, 20).unwrap();
        assert_eq!(format_java_pattern("yyyy-MM-dd", &dt), "2023-11-14");
        assert_eq!(format_java_pattern("yy/M/d HH:mm:ss", &dt), "23/11/14 22:13:20");
        assert_eq!(format_java_pattern("hh:mm a", &dt), "10:13 PM");
        assert_eq!(format_java_pattern("'年'yyyy", &dt), "年2023");
    }

    #[test]
    fn base64_flags() {
        // NO_WRAP(java.base64Encode 的默认)
        assert_eq!(base64_encode_bytes(b"abc", B64_NO_WRAP), "YWJj");
        // DEFAULT 带换行(末尾也有)
        assert_eq!(base64_encode_bytes(b"abc", B64_DEFAULT), "YWJj\n");
        assert_eq!(base64_decode_bytes("YWJj").unwrap(), b"abc");
        // 缺 padding / URL 安全字母表都认
        assert_eq!(base64_decode_bytes("YWJjZA").unwrap(), b"abcd");
    }

    #[test]
    fn encode_uri_form_style() {
        // URLEncoder:空格是 +,中文按字符集
        assert_eq!(encode_uri("中 文", None), "%E4%B8%AD+%E6%96%87");
        assert_eq!(encode_uri("中 文", Some("GBK")), "%D6%D0+%CE%C4");
        // 不认识的字符集 → 真身抛,被吞成空串
        assert_eq!(encode_uri("x", Some("no-such-charset")), "");
    }
}

// ---------- 摘要 / HMAC ----------

/// hutool `DigestUtil.digester(algorithm)` 认的名字 → 我们的实现。
///
/// hutool 底下是 `MessageDigest.getInstance(algorithm)`,**JCA 的算法名不区分
/// 大小写**,且 `SHA` / `SHA1` 都是 `SHA-1` 的别名。不认识的名字 hutool 包成
/// `CryptoException`(裁判侧归到 `host:CryptoException`)。
fn digest_bytes(algorithm: &str, data: &[u8]) -> Result<Vec<u8>, String> {
    use sha1::Digest as _;
    let a = algorithm.to_ascii_uppercase().replace('-', "");
    Ok(match a.as_str() {
        "MD5" => {
            let mut h = md5::Md5::new();
            h.update(data);
            h.finalize().to_vec()
        }
        "SHA" | "SHA1" => {
            let mut h = sha1::Sha1::new();
            h.update(data);
            h.finalize().to_vec()
        }
        "SHA224" => {
            let mut h = sha2::Sha224::new();
            h.update(data);
            h.finalize().to_vec()
        }
        "SHA256" => {
            let mut h = sha2::Sha256::new();
            h.update(data);
            h.finalize().to_vec()
        }
        "SHA384" => {
            let mut h = sha2::Sha384::new();
            h.update(data);
            h.finalize().to_vec()
        }
        "SHA512" => {
            let mut h = sha2::Sha512::new();
            h.update(data);
            h.finalize().to_vec()
        }
        _ => return Err(HOST_CRYPTO_EXCEPTION.into()),
    })
}

/// 算法名不认识时,真身(hutool)包成 `CryptoException`。两侧按类别比。
pub const HOST_CRYPTO_EXCEPTION: &str = "«host:CryptoException»";

/// `JsEncodeUtils.digestHex(data, algorithm)` = `DigestUtil.digester(a).digestHex(data)`
/// —— hutool 的 `digestHex(String)` 按 **UTF-8** 取字节。
pub fn digest_hex(data: &str, algorithm: &str) -> Result<String, String> {
    digest_bytes(algorithm, data.as_bytes()).map(hex::encode)
}

/// `JsEncodeUtils.HMacHex(data, algorithm, key)` = `HMac(a, key.toByteArray()).digestHex(data)`。
///
/// hutool 的 `HMac` 底下是 `Mac.getInstance(algorithm)`,名字形如 `HmacMD5` /
/// `HmacSHA256`(同样不区分大小写)。这里按 RFC 2104 直接算 —— 分组长度取各
/// 摘要算法自己的 block size(MD5/SHA-1/SHA-224/SHA-256 是 64,SHA-384/512 是 128)。
pub fn hmac_hex(data: &str, algorithm: &str, key: &str) -> Result<String, String> {
    hmac_bytes(algorithm, key.as_bytes(), data.as_bytes(), CryptoFlavor::Hutool).map(hex::encode)
}

/// HMAC 本体(RFC 2104)。`Hmac<摘要>` 的名字形如 `HmacMD5` / `HmacSHA256`,
/// **不区分大小写**;分组长按各摘要算法自己的 block size
/// (MD5/SHA-1/SHA-224/SHA-256 是 64,SHA-384/512 是 128)。
///
/// 两条路共用:`java.HMacHex` 走 hutool(错了包 `CryptoException`),
/// LiveConnect 的 `javax.crypto.Mac` 直连 JCE(`NoSuchAlgorithmException`)。
pub fn hmac_bytes(
    algorithm: &str,
    key: &[u8],
    data: &[u8],
    flavor: CryptoFlavor,
) -> Result<Vec<u8>, String> {
    let no_alg = || match flavor {
        CryptoFlavor::Hutool => HOST_CRYPTO_EXCEPTION.to_string(),
        CryptoFlavor::Jce => "«host:NoSuchAlgorithmException»".to_string(),
    };
    let a = algorithm.to_ascii_uppercase().replace('-', "");
    let inner = a.strip_prefix("HMAC").ok_or_else(no_alg)?;
    let block = match inner {
        "MD5" | "SHA" | "SHA1" | "SHA224" | "SHA256" => 64usize,
        "SHA384" | "SHA512" => 128,
        _ => return Err(no_alg()),
    };
    // RFC 2104:key 比分组长就先摘要,再右补零到分组长
    let mut k = key.to_vec();
    if k.len() > block {
        k = digest_bytes(inner, &k).map_err(|_| no_alg())?;
    }
    k.resize(block, 0);
    let mut ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    ipad.extend_from_slice(data);
    let inner_digest = digest_bytes(inner, &ipad).map_err(|_| no_alg())?;
    let mut outer = opad;
    outer.extend_from_slice(&inner_digest);
    digest_bytes(inner, &outer).map_err(|_| no_alg())
}

/// LiveConnect 的 `java.security.MessageDigest.getInstance(a).digest(bytes)`。
/// 不认识的算法名是 JCE 自己的 `NoSuchAlgorithmException`(不经 hutool)。
pub fn jce_digest(algorithm: &str, data: &[u8]) -> Result<Vec<u8>, String> {
    digest_bytes(algorithm, data).map_err(|_| "«host:NoSuchAlgorithmException»".to_string())
}

// ---------- 随机 ----------

/// **差分定死的随机字节流**,与裁判侧 `jsharness/DeterministicRandom` 逐字对齐:
///
/// ```text
/// block(i) = SHA-256( "rubato-difftest" ‖ be_u64(i) )
/// ```
///
/// 从 i = 0 开始按需拼接。产品侧当然要用真随机 —— 这条路只在
/// [`RandomSource::Difftest`] 下走,理由见
/// `fixtures/cases/js-host/README.md`「确定性垫片」。
pub fn deterministic_random_bytes_at(counter: u64) -> [u8; 32] {
    use sha2::Digest as _;
    let mut h = sha2::Sha256::new();
    h.update(b"rubato-difftest");
    h.update(counter.to_be_bytes());
    h.finalize().into()
}

/// `UUID.randomUUID().toString()`:取 16 字节,按 RFC 4122 打上 version 4 与
/// variant 位,再按 `8-4-4-4-12` 十六进制拼。
pub fn uuid_from_bytes(b: &[u8; 16]) -> String {
    let mut b = *b;
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // IETF variant
    let h = hex::encode(b);
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

// ---------- 章节号 ----------

/// `StringUtils.fullToHalf`:全角空格 → 半角,`！`..`～`(U+FF01..U+FF5E)→ ASCII
pub fn full_to_half(input: &str) -> String {
    input
        .chars()
        .map(|c| match c as u32 {
            12288 => ' ',
            n @ 65281..=65374 => char::from_u32(n - 65248).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// `StringUtils.chnMap`:中文数字 → 值
fn chn_value(c: char) -> Option<i64> {
    const CN: &str = "零一二三四五六七八九十";
    const CN2: &str = "〇壹贰叁肆伍陆柒捌玖拾";
    if let Some(i) = CN.chars().position(|x| x == c) {
        return Some(i as i64);
    }
    if let Some(i) = CN2.chars().position(|x| x == c) {
        return Some(i as i64);
    }
    Some(match c {
        '两' => 2,
        '百' | '佰' => 100,
        '千' | '仟' => 1000,
        '万' => 10000,
        '亿' => 100000000,
        _ => return None,
    })
}

/// `StringUtils.chineseNumToInt`。逐行照搬,含两处怪癖:
/// - 「一零二五」那条分支的正则是 `^[…]$`(**只匹配单字符**),
///   所以 `cn.size > 1` 时它**永远进不去** —— 真身的死代码,照留;
/// - 认不出的字在真身里是 `ChnMap[c]!!` 的 NPE,被 `runCatching` 吞成 **-1**。
pub fn chinese_num_to_int(ch_num: &str) -> i64 {
    let cn: Vec<char> = ch_num.chars().collect();
    let mut result: i64 = 0;
    let mut tmp: i64 = 0;
    let mut billion: i64 = 0;
    for i in 0..cn.len() {
        let Some(tmp_num) = chn_value(cn[i]) else { return -1 };
        if tmp_num == 100000000 {
            result += tmp;
            result *= tmp_num;
            billion = billion * 100000000 + result;
            result = 0;
            tmp = 0;
        } else if tmp_num == 10000 {
            result += tmp;
            result *= tmp_num;
            tmp = 0;
        } else if tmp_num >= 10 {
            if tmp == 0 {
                tmp = 1;
            }
            result += tmp_num * tmp;
            tmp = 0;
        } else {
            let prev = if i >= 2 { chn_value(cn[i - 1]) } else { None };
            tmp = match prev {
                Some(p) if i == cn.len() - 1 && p > 10 => tmp_num * p / 10,
                _ => tmp * 10 + tmp_num,
            };
        }
    }
    result + tmp + billion
}

/// `StringUtils.stringToInt`:先按十进制整数试,不行再当中文数字;`null` → -1
pub fn string_to_int(s: &str) -> i64 {
    let num: String = full_to_half(s).chars().filter(|c| !c.is_whitespace()).collect();
    // Java 的 Integer.parseInt:可带正负号,不吃空白与下划线,溢出即失败
    if let Ok(v) = num.parse::<i32>() {
        return v as i64;
    }
    chinese_num_to_int(&num)
}

/// `JsExtensions.toNumChapter(s)`:`(第)(.+?)(章)` 的第一处匹配 →
/// `"${group(1)}${stringToInt(group(2))}${group(3)}"`。
///
/// **注意它返回的只是重拼的那一段**(永远是 `第<数>章`),匹配之外的文字
/// **全丢**;匹配不到才原样返回整串。真身就是这么写的,别"顺手修好"。
pub fn to_num_chapter(s: &str) -> String {
    // Java 的 `Pattern.compile("(第)(.+?)(章)")` + `find()`:`.` 不吃行终止符
    let chars: Vec<char> = s.chars().collect();
    for start in 0..chars.len() {
        if chars[start] != '第' {
            continue;
        }
        // 非贪婪:从最短的中间段开始找最近的「章」
        for end in (start + 2)..chars.len() {
            let mid: String = chars[start + 1..end].iter().collect();
            if mid.chars().any(|c| matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')) {
                break;
            }
            if chars[end] == '章' {
                return format!("第{}章", string_to_int(&mid));
            }
        }
    }
    s.to_string()
}

// ---------- 对称加解密(hutool SymmetricCrypto / SymmetricCryptoAndroid)----------
//
// 书源里 `java.aes*` / `java.des*` / `java.createSymmetricCrypto` 全都汇到
// `createSymmetricCrypto(transformation, key, iv)` 这一处(JsEncodeUtils.kt L45),
// 底下是 hutool 的 `SymmetricCrypto` —— 也就是 JCE 的 `Cipher.getInstance`。
// 故这里只做一份核心,上面那些名字都是它的薄壳。
//
// **错误口径**:hutool 把 JCE 的一切异常都包成 `CryptoException`
// (`SymmetricCrypto.encrypt/decrypt` 里 `catch (Exception e) { throw new CryptoException(e); }`),
// 所以密钥长度不对、块大小不对、补码不对,裁判侧一律是 `host:CryptoException`。
// 直接走 LiveConnect 调 JCE 的那一支不经 hutool,类别是 JCE 自己的
// (`IllegalBlockSizeException` 等)—— 那是另一条路,不在这里。

/// `transformation` 三段式:`算法/模式/补码`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymAlgo {
    Aes,
    Des,
    DesEde,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymMode {
    Ecb,
    Cbc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymPadding {
    /// `PKCS5Padding` / `PKCS7Padding` —— 对分组密码是同一回事
    Pkcs,
    None,
    /// hutool 自己实现的补零(JCE 没有这个名字)
    Zero,
}

pub struct SymmetricCrypto {
    algo: SymAlgo,
    mode: SymMode,
    padding: SymPadding,
    key: Vec<u8>,
    iv: Option<Vec<u8>>,
    flavor: CryptoFlavor,
}

/// **同一段算术,两套错误类别**。
///
/// 走 hutool 的那条路(`java.aes*` / `des*` / `createSymmetricCrypto`)里,
/// `SymmetricCrypto.encrypt/decrypt` 把 JCE 的一切异常都 `catch` 成
/// `CryptoException`;而书源用 LiveConnect **直接调 `javax.crypto.Cipher`**
/// 时不经 hutool,抛的是 JCE 自己的类。分组密码本体是同一份,差的只有这一层皮。
///
/// **有一位差分在这台裁判上问不出答案**(M3q,记豁免不记欠账):hutool 5.8.22 的
/// `KeyUtil.generateKey(algorithm, key)` 在非 PBE / 非 DES 分支上把**整条
/// transformation 当算法名**塞进 `SecretKeySpec`(字节码可查,没有
/// `getMainAlgorithm`)。于是密钥的 `getAlgorithm()` 是 `AES/CBC/PKCS5Padding` ——
/// 裁判跑在 JVM 上,SunJCE 的 `AESCipher` 查这个名字并拒收;真身跑在 Android 上,
/// Conscrypt 只查密钥长度、照跑。语料 17 个源这么写,故**这里照 Android 放行**,
/// 判据侧记豁免(`fixtures/cases/js-host/exemptions.json` 的
/// `js-hut-aes-full-transformation`)。DES / DESede 不受影响(hutool 的 DES 分支
/// 走 `SecretKeyFactory` + `getMainAlgorithm`)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoFlavor {
    /// hutool:一律 `CryptoException`
    Hutool,
    /// LiveConnect 直连 JCE:`IllegalBlockSizeException` 等
    Jce,
}

/// JCE 的异常类别(差分只比类别,不比消息)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrKind {
    /// `Cipher.getInstance` 不认识这个 transformation。
    /// 注意补码名不认识时 JCE 抛的**也是**这个(探针 `lc-err-nopad`:
    /// `AES/CBC/NoSuchPadding` → NoSuchAlgorithmException,不是 NoSuchPaddingException)
    NoSuchAlgorithm,
    /// 密钥长度不合法 —— JCE 在 `init` 那一步炸
    InvalidKey,
    /// IV 长度不等于分组长 —— 同样在 `init` 炸
    InvalidAlgParam,
    /// 输入长度不是分组长的整数倍
    IllegalBlockSize,
    /// 解出来的补码不合法(密钥/IV 错时的典型现象)
    BadPadding,
}

fn crypto_err(flavor: CryptoFlavor, kind: ErrKind) -> String {
    match flavor {
        CryptoFlavor::Hutool => HOST_CRYPTO_EXCEPTION.to_string(),
        CryptoFlavor::Jce => match kind {
            ErrKind::NoSuchAlgorithm => "«host:NoSuchAlgorithmException»".into(),
            ErrKind::InvalidKey => "«host:InvalidKeyException»".into(),
            ErrKind::InvalidAlgParam => "«host:InvalidAlgorithmParameterException»".into(),
            ErrKind::IllegalBlockSize => "«host:IllegalBlockSizeException»".into(),
            ErrKind::BadPadding => "«host:BadPaddingException»".into(),
        },
    }
}

impl SymmetricCrypto {
    /// `createSymmetricCrypto(transformation, key, iv)`。
    /// 密钥长度不合法在真身里要到 `Cipher.init` 才炸,但一样是 CryptoException,
    /// 故这里提前判掉。
    pub fn new(transformation: &str, key: &[u8], iv: Option<&[u8]>) -> Result<Self, String> {
        Self::new_with(CryptoFlavor::Hutool, transformation, key, iv)
    }

    /// LiveConnect 那一支:`Cipher.getInstance` + `init` 的错误类别按 JCE 分。
    /// **两步的边界要对**:transformation 不认识是 `getInstance` 抛的,
    /// 密钥/IV 长度是 `init` 抛的 —— 调用方(`live_connect`)照这个边界拆。
    pub fn new_jce(transformation: &str, key: &[u8], iv: Option<&[u8]>) -> Result<Self, String> {
        Self::new_with(CryptoFlavor::Jce, transformation, key, iv)
    }

    fn new_with(
        flavor: CryptoFlavor,
        transformation: &str,
        key: &[u8],
        iv: Option<&[u8]>,
    ) -> Result<Self, String> {
        let (algo, mode, padding) = parse_transformation(transformation)
            .ok_or_else(|| crypto_err(flavor, ErrKind::NoSuchAlgorithm))?;
        let key_ok = match algo {
            SymAlgo::Aes => matches!(key.len(), 16 | 24 | 32),
            SymAlgo::Des => key.len() == 8,
            SymAlgo::DesEde => matches!(key.len(), 16 | 24),
        };
        if !key_ok {
            return Err(crypto_err(flavor, ErrKind::InvalidKey));
        }
        let iv = iv.filter(|v| !v.is_empty()).map(|v| v.to_vec());
        Ok(SymmetricCrypto { algo, mode, padding, key: key.to_vec(), iv, flavor })
    }

    fn err(&self, kind: ErrKind) -> String {
        crypto_err(self.flavor, kind)
    }

    fn block_size(&self) -> usize {
        match self.algo {
            SymAlgo::Aes => 16,
            SymAlgo::Des | SymAlgo::DesEde => 8,
        }
    }

    /// CBC 的 IV:长度必须等于分组长。JCE 在 `init` 那一步抛
    /// `InvalidAlgorithmParameterException`,hutool 把它包成 CryptoException。
    /// ECB 不是「忽略 IV」而是**给了就报错**,见 [`Self::check_init`]。
    fn iv_block(&self) -> Result<Vec<u8>, String> {
        let bs = self.block_size();
        match &self.iv {
            Some(v) if v.len() == bs => Ok(v.clone()),
            // 没给 IV 时 JCE 的 CBC 加密会自己随机生成 —— 那不可差分,
            // 且书源都会显式给。这里按「缺 IV 即错」处理。
            _ => Err(self.err(ErrKind::InvalidAlgParam)),
        }
    }

    /// `Cipher.init` 那一步能查出来的错(密钥长度已在 `new_with` 里查过,
    /// 这里补 IV 那两条)。LiveConnect 侧在 `init` 调它,让错误落在与真身
    /// 相同的那一行;hutool 那条路没有单独的 init 入口(`SymmetricCrypto`
    /// 每次 encrypt/decrypt 自己 `initMode`),故两头都从这里进。
    ///
    /// **ECB 给了 IV 是错,不是忽略**:hutool 只要 `iv != null` 就传
    /// `IvParameterSpec`,而 ECB 的 `Cipher.init` 收到 params 就抛
    /// `InvalidAlgorithmParameterException: ECB mode cannot use IV`
    /// (探针 `js-hut-ecb-iv-des` / `js-hut-ecb-iv-bare-aes` / `js-lc-err-ecb-iv`;
    /// JVM 与 Android 的 Conscrypt 在这一位上一致,与 [`CryptoFlavor`] 那段注释
    /// 里记的「裁判环境差」不是一回事)。
    pub fn check_init(&self) -> Result<(), String> {
        match self.mode {
            SymMode::Cbc => self.iv_block().map(|_| ()),
            SymMode::Ecb if self.iv.is_some() => Err(self.err(ErrKind::InvalidAlgParam)),
            SymMode::Ecb => Ok(()),
        }
    }

    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        self.check_init()?;
        let bs = self.block_size();
        let mut buf = data.to_vec();
        match self.padding {
            SymPadding::Pkcs => {
                let pad = bs - (buf.len() % bs);
                buf.extend(std::iter::repeat_n(pad as u8, pad));
            }
            SymPadding::Zero => {
                let rem = buf.len() % bs;
                if rem != 0 {
                    buf.extend(std::iter::repeat_n(0u8, bs - rem));
                }
            }
            SymPadding::None => {
                if !buf.len().is_multiple_of(bs) {
                    return Err(self.err(ErrKind::IllegalBlockSize));
                }
            }
        }
        self.run_blocks(&mut buf, true)?;
        Ok(buf)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        self.check_init()?;
        let bs = self.block_size();
        // **空输入解密是空串,不是错**:JCE 的 `CipherCore.doFinal` 先判
        // `totalLen % blockSize`(0 过得去),再让 `PKCS5Padding.unpad` 对
        // `len == 0` 直接返回 0 —— 于是 `doFinal(new byte[0])` 交回空数组。
        // 语料里这一位真的会走到:书源拿明文页喂 `Base64.decode(…, 2)`,
        // 表外字符被 AOSP 的解码表全跳过 → 0 字节 → 再进 doFinal
        // (`pb02492`「🔰笔趣阁.pysmei」的 `<js>` 就是这条路)。
        if data.is_empty() {
            return Ok(Vec::new());
        }
        if !data.len().is_multiple_of(bs) {
            return Err(self.err(ErrKind::IllegalBlockSize));
        }
        let mut buf = data.to_vec();
        self.run_blocks(&mut buf, false)?;
        match self.padding {
            SymPadding::Pkcs => {
                let pad = *buf.last().expect("非空") as usize;
                if pad == 0 || pad > bs || pad > buf.len() {
                    return Err(self.err(ErrKind::BadPadding));
                }
                if buf[buf.len() - pad..].iter().any(|&b| b as usize != pad) {
                    return Err(self.err(ErrKind::BadPadding));
                }
                buf.truncate(buf.len() - pad);
            }
            // hutool 的 ZeroPadding 解密后**不去零**(JCE 侧按 NoPadding 跑),
            // NoPadding 同样原样返回
            SymPadding::Zero | SymPadding::None => {}
        }
        Ok(buf)
    }

    /// 就地跑 ECB / CBC。分组密码本体来自 `aes` / `des` crate。
    fn run_blocks(&self, buf: &mut [u8], encrypt: bool) -> Result<(), String> {
        use cipher::KeyInit;
        let iv = match self.mode {
            SymMode::Ecb => vec![0u8; self.block_size()],
            SymMode::Cbc => self.iv_block()?,
        };
        let bad = |_| self.err(ErrKind::InvalidKey);
        match (self.algo, self.key.len()) {
            (SymAlgo::Aes, 16) => run_with(
                &aes::Aes128::new_from_slice(&self.key).map_err(bad)?,
                buf,
                encrypt,
                self.mode,
                &iv,
            ),
            (SymAlgo::Aes, 24) => run_with(
                &aes::Aes192::new_from_slice(&self.key).map_err(bad)?,
                buf,
                encrypt,
                self.mode,
                &iv,
            ),
            (SymAlgo::Aes, 32) => run_with(
                &aes::Aes256::new_from_slice(&self.key).map_err(bad)?,
                buf,
                encrypt,
                self.mode,
                &iv,
            ),
            (SymAlgo::Des, _) => run_with(
                &des::Des::new_from_slice(&self.key).map_err(bad)?,
                buf,
                encrypt,
                self.mode,
                &iv,
            ),
            (SymAlgo::DesEde, len) => {
                // DESede 的 16 字节密钥是 K1‖K2,等价于 K1‖K2‖K1(JCE 同此)
                let mut k = self.key.clone();
                if len == 16 {
                    k.extend_from_slice(&self.key[..8]);
                }
                run_with(
                    &des::TdesEde3::new_from_slice(&k).map_err(bad)?,
                    buf,
                    encrypt,
                    self.mode,
                    &iv,
                )
            }
            _ => return Err(self.err(ErrKind::InvalidKey)),
        }
        Ok(())
    }
}

/// `算法/模式/补码` 三段式。认不出来返回 `None` —— 调用方按自己那条路
/// (hutool / JCE)决定包成哪个异常类。
///
/// **CFB / OFB / CTR 显式不认**:真身两条路都能跑,被测侧算不了。宁可在差分上
/// 现形,也不要静默算错(见 M2d 的决定)。
fn parse_transformation(transformation: &str) -> Option<(SymAlgo, SymMode, SymPadding)> {
    let mut parts = transformation.split('/');
    let algo = match parts.next().unwrap_or("").to_ascii_uppercase().as_str() {
        "AES" => SymAlgo::Aes,
        "DES" => SymAlgo::Des,
        "DESEDE" | "TRIPLEDES" => SymAlgo::DesEde,
        _ => return None,
    };
    let mode = match parts.next().unwrap_or("ECB").to_ascii_uppercase().as_str() {
        "ECB" => SymMode::Ecb,
        "CBC" => SymMode::Cbc,
        _ => return None,
    };
    let padding = match parts.next().unwrap_or("PKCS5PADDING").to_ascii_uppercase().as_str() {
        "PKCS5PADDING" | "PKCS7PADDING" => SymPadding::Pkcs,
        "NOPADDING" => SymPadding::None,
        "ZEROPADDING" => SymPadding::Zero,
        _ => return None,
    };
    Some((algo, mode, padding))
}

/// ECB / CBC 的分组循环。`iv` 在 ECB 下是全零且不参与(留着只为一个签名)。
fn run_with<C>(c: &C, buf: &mut [u8], encrypt: bool, mode: SymMode, iv: &[u8])
where
    C: cipher::BlockCipherEncrypt + cipher::BlockCipherDecrypt,
{
    use cipher::array::typenum::Unsigned as _;
    let bs = <C as cipher::BlockSizeUser>::BlockSize::USIZE;
    let mut prev = iv.to_vec();
    prev.resize(bs, 0);
    let mut i = 0;
    while i + bs <= buf.len() {
        let block = &mut buf[i..i + bs];
        if encrypt {
            if mode == SymMode::Cbc {
                for k in 0..bs {
                    block[k] ^= prev[k];
                }
            }
            c.encrypt_block((&mut *block).try_into().expect("块长 = BlockSize"));
            if mode == SymMode::Cbc {
                prev.copy_from_slice(block);
            }
        } else {
            let ct = block.to_vec();
            c.decrypt_block((&mut *block).try_into().expect("块长 = BlockSize"));
            if mode == SymMode::Cbc {
                for k in 0..bs {
                    block[k] ^= prev[k];
                }
                prev.copy_from_slice(&ct);
            }
        }
        i += bs;
    }
}

/// `SymmetricCryptoAndroid.decrypt(String)`:**先看是不是纯十六进制**
/// (`String.isHex()` —— 全部字符落在 `0-9A-Fa-f`),是就 hex 解,否则 base64
/// (StringExtensions.kt L83 / SymmetricCryptoAndroid.kt L38)。
///
/// 注意 `isHex` 对**空串**返回 true(`all {}` 的空真),且不看长度是否为偶数。
pub fn sym_decode_input(data: &str) -> Result<Vec<u8>, String> {
    // **空串在这条路上是错,不是空**:`HexUtil.decodeHex("")` 与
    // `Base64.decode("")` 在 hutool 里**都返回 null**(两处都是 `isEmpty` 判空),
    // 于是 `cipher.doFinal(null)` 抛 `IllegalArgumentException: Null input buffer`
    // → 包成 CryptoException(探针 `js-hut-dec-empty`)。
    // 只有**空**这一档:`" "` / `"\n"` 走 base64 的宽松表 → 0 字节 → 交空串
    // (探针 `js-hut-dec-blank`),与 LiveConnect 那条 `doFinal(new byte[0])`
    // 一样是空 —— 三条边界挨着,别合并。
    if data.is_empty() {
        return Err(HOST_CRYPTO_EXCEPTION.to_string());
    }
    let is_hex = data.chars().all(|c| c.is_ascii_hexdigit());
    if is_hex {
        return hex::decode(data).map_err(|_| HOST_CRYPTO_EXCEPTION.to_string());
    }
    // hutool 的 `Base64.decode` 是**宽松**的:不认识的字符直接跳过,不报错
    // (`Base64Decoder` 用一张查找表,表外的字符 continue)。所以喂给它一段
    // 中文也不会抛 —— 抛是后面 `decrypt` 因长度不对才抛的 CryptoException。
    // 严格解码会把错误类别钉错(实测 js-corpus-e1d3254e62)。
    Ok(base64_decode_lenient(data))
}

/// `aes*` / `des*` 那一排薄壳的口径。名字里的 `Base64` 说的是**输入**是不是
/// base64,不是输出 —— 真身其实全走 `SymmetricCryptoAndroid.decrypt(String)`
/// 的「hex 还是 base64」自动判定,故两者在被测侧是同一条路
/// (`aesDecodeToString` 那条例外:它收的是**原始串的字节**)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymOp {
    /// `decryptStr(str)`:输入按 hex/base64 自动判定
    DecryptStr,
    /// `encrypt(data)` 后按 UTF-8 造串(真身 `String(...encrypt(data))`)
    EncryptStr,
    /// `encryptBase64(data)`
    EncryptBase64,
}

/// `aes*/des*` 薄壳的统一实现
pub fn symmetric_str_op(
    op: SymOp,
    data: &str,
    key: &[u8],
    transformation: &str,
    iv: &[u8],
) -> Result<Option<String>, String> {
    let c = SymmetricCrypto::new(transformation, key, if iv.is_empty() { None } else { Some(iv) })?;
    Ok(Some(match op {
        SymOp::DecryptStr => {
            let bytes = sym_decode_input(data)?;
            String::from_utf8_lossy(&c.decrypt(&bytes)?).into_owned()
        }
        SymOp::EncryptStr => String::from_utf8_lossy(&c.encrypt(data.as_bytes())?).into_owned(),
        SymOp::EncryptBase64 => base64_encode_bytes(&c.encrypt(data.as_bytes())?, B64_NO_WRAP),
    }))
}

// ---------- java.security 的 RSA 一支 ----------

/// `SHA256withRSA` 这类算法名 → (摘要算法名, PKCS#1 v1.5 的 DigestInfo 前缀)。
///
/// 走「先自己算摘要、再拿裸摘要签」这条路而不是 `SigningKey::<Sha256>`:
/// `rsa` 0.9 绑的是 digest 0.10,而本 crate 的 sha1/sha2 是 0.11 —— 两套 trait
/// 对不上。摘要那一半复用 [`jce_digest`](本 crate 里唯一一份摘要表),
/// 签名这一半只需要 DigestInfo 的前缀(RFC 8017 §9.2 那张常量表)。
fn rsa_digest_info(alg: &str) -> Option<(&'static str, &'static [u8])> {
    // Java 的算法名不分大小写,连字符可有可无(`SHA-256withRSA` 也认)
    let norm = alg.to_ascii_lowercase().replace('-', "");
    let d = norm.strip_suffix("withrsa")?;
    Some(match d {
        "md5" => (
            "MD5",
            &[
                0x30, 0x20, 0x30, 0x0c, 0x06, 0x08, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x02, 0x05,
                0x05, 0x00, 0x04, 0x10,
            ],
        ),
        "sha1" | "sha" => (
            "SHA-1",
            &[
                0x30, 0x21, 0x30, 0x09, 0x06, 0x05, 0x2b, 0x0e, 0x03, 0x02, 0x1a, 0x05, 0x00, 0x04,
                0x14,
            ],
        ),
        "sha224" => (
            "SHA-224",
            &[
                0x30, 0x2d, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x04, 0x05, 0x00, 0x04, 0x1c,
            ],
        ),
        "sha256" => (
            "SHA-256",
            &[
                0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x01, 0x05, 0x00, 0x04, 0x20,
            ],
        ),
        "sha384" => (
            "SHA-384",
            &[
                0x30, 0x41, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x02, 0x05, 0x00, 0x04, 0x30,
            ],
        ),
        "sha512" => (
            "SHA-512",
            &[
                0x30, 0x51, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                0x03, 0x05, 0x00, 0x04, 0x40,
            ],
        ),
        _ => return None,
    })
}

fn pkcs1v15_scheme(alg: &str) -> Result<(rsa::Pkcs1v15Sign, &'static str), String> {
    let (digest, prefix) =
        rsa_digest_info(alg).ok_or_else(|| "«host:NoSuchAlgorithmException»".to_string())?;
    let hash_len = digest_bytes(digest, b"").map_err(|_| "«host:NoSuchAlgorithmException»")?.len();
    Ok((rsa::Pkcs1v15Sign { hash_len: Some(hash_len), prefix: prefix.into() }, digest))
}

/// `Signature.getInstance(alg)` + `initSign(privateKey)` + `update(data)` + `sign()`
/// 一条龙。真身那三步是有状态的,但书源全都是「装好私钥、喂一段、签一次」——
/// 状态层留在 JS 侧([`crate::live_connect::LIVE_CONNECT_JS`]),这里只做算。
///
/// `pkcs8` 是 `PKCS8EncodedKeySpec` 拿到的那串 DER(书源直接写成字节字面量)。
/// 语料 1704 源里走这条的有 **2 个**(西瓜小说:`sign` 请求头是 `SHA256WithRSA`
/// 的签名再过一层自写 base64)—— 不接就是这两个源在 Rubato 上直接失效。
///
/// 错误按 Java 的类别分:算法名不认得 `NoSuchAlgorithmException`,私钥解不开
/// `InvalidKeySpecException`(真身在 `KeyFactory.generatePrivate` 那一步抛),
/// 签名本身失败 `SignatureException`。
pub fn rsa_sign(alg: &str, pkcs8: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    use rsa::pkcs8::DecodePrivateKey as _;
    let (scheme, digest) = pkcs1v15_scheme(alg)?;
    let key = rsa::RsaPrivateKey::from_pkcs8_der(pkcs8)
        .map_err(|_| "«host:InvalidKeySpecException»".to_string())?;
    let hashed = jce_digest(digest, data)?;
    key.sign(scheme, &hashed).map_err(|_| "«host:SignatureException»".to_string())
}
