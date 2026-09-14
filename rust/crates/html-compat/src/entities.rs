//! commons-text `StringEscapeUtils.unescapeHtml4` 的等价实现。
//!
//! AnalyzeRule.getString 在结果含 `&` 时调它做反转义——注意这是 **commons-text
//! 的 HTML4 表**,不是 jsoup/WHATWG 的 HTML5 表:
//! - 只认带分号的实体(`&amp` 不还原),不含 `&apos;`;
//! - 数字实体默认 `semiColonRequired`,十进制串按十六进制字符集扫描后再按十进制
//!   解析,于是 `&#12a;` 走 NumberFormatException 分支原样保留;
//! - 查表是「同一起点由长到短」的贪婪匹配(LookupTranslator)。
//!
//! 表由 commons-text 1.15.0 的 EntityArrays(BASIC/ISO8859_1/HTML40_EXTENDED)
//! 直接 dump 生成,勿手改。

const SHORTEST: usize = 4;
const LONGEST: usize = 10;

/// (实体, 展开)——252 条,与 commons-text 1.15.0 逐条一致
#[rustfmt::skip] // 手排的分列表:rustfmt 会拆成一行一个
static HTML4: [(&str, &str); 252] = [
    ("&amp;", "&"),
    ("&gt;", ">"),
    ("&lt;", "<"),
    ("&quot;", "\""),
    ("&AElig;", "Æ"),
    ("&Aacute;", "Á"),
    ("&Acirc;", "Â"),
    ("&Agrave;", "À"),
    ("&Aring;", "Å"),
    ("&Atilde;", "Ã"),
    ("&Auml;", "Ä"),
    ("&Ccedil;", "Ç"),
    ("&ETH;", "Ð"),
    ("&Eacute;", "É"),
    ("&Ecirc;", "Ê"),
    ("&Egrave;", "È"),
    ("&Euml;", "Ë"),
    ("&Iacute;", "Í"),
    ("&Icirc;", "Î"),
    ("&Igrave;", "Ì"),
    ("&Iuml;", "Ï"),
    ("&Ntilde;", "Ñ"),
    ("&Oacute;", "Ó"),
    ("&Ocirc;", "Ô"),
    ("&Ograve;", "Ò"),
    ("&Oslash;", "Ø"),
    ("&Otilde;", "Õ"),
    ("&Ouml;", "Ö"),
    ("&THORN;", "Þ"),
    ("&Uacute;", "Ú"),
    ("&Ucirc;", "Û"),
    ("&Ugrave;", "Ù"),
    ("&Uuml;", "Ü"),
    ("&Yacute;", "Ý"),
    ("&aacute;", "á"),
    ("&acirc;", "â"),
    ("&acute;", "´"),
    ("&aelig;", "æ"),
    ("&agrave;", "à"),
    ("&aring;", "å"),
    ("&atilde;", "ã"),
    ("&auml;", "ä"),
    ("&brvbar;", "¦"),
    ("&ccedil;", "ç"),
    ("&cedil;", "¸"),
    ("&cent;", "¢"),
    ("&copy;", "©"),
    ("&curren;", "¤"),
    ("&deg;", "°"),
    ("&divide;", "÷"),
    ("&eacute;", "é"),
    ("&ecirc;", "ê"),
    ("&egrave;", "è"),
    ("&eth;", "ð"),
    ("&euml;", "ë"),
    ("&frac12;", "½"),
    ("&frac14;", "¼"),
    ("&frac34;", "¾"),
    ("&iacute;", "í"),
    ("&icirc;", "î"),
    ("&iexcl;", "¡"),
    ("&igrave;", "ì"),
    ("&iquest;", "¿"),
    ("&iuml;", "ï"),
    ("&laquo;", "«"),
    ("&macr;", "¯"),
    ("&micro;", "µ"),
    ("&middot;", "·"),
    ("&nbsp;", " "),
    ("&not;", "¬"),
    ("&ntilde;", "ñ"),
    ("&oacute;", "ó"),
    ("&ocirc;", "ô"),
    ("&ograve;", "ò"),
    ("&ordf;", "ª"),
    ("&ordm;", "º"),
    ("&oslash;", "ø"),
    ("&otilde;", "õ"),
    ("&ouml;", "ö"),
    ("&para;", "¶"),
    ("&plusmn;", "±"),
    ("&pound;", "£"),
    ("&raquo;", "»"),
    ("&reg;", "®"),
    ("&sect;", "§"),
    ("&shy;", "\u{AD}"), // 软连字符(不可见字符,写转义免得肉眼看不出来)
    ("&sup1;", "¹"),
    ("&sup2;", "²"),
    ("&sup3;", "³"),
    ("&szlig;", "ß"),
    ("&thorn;", "þ"),
    ("&times;", "×"),
    ("&uacute;", "ú"),
    ("&ucirc;", "û"),
    ("&ugrave;", "ù"),
    ("&uml;", "¨"),
    ("&uuml;", "ü"),
    ("&yacute;", "ý"),
    ("&yen;", "¥"),
    ("&yuml;", "ÿ"),
    ("&Alpha;", "Α"),
    ("&Beta;", "Β"),
    ("&Chi;", "Χ"),
    ("&Dagger;", "‡"),
    ("&Delta;", "Δ"),
    ("&Epsilon;", "Ε"),
    ("&Eta;", "Η"),
    ("&Gamma;", "Γ"),
    ("&Iota;", "Ι"),
    ("&Kappa;", "Κ"),
    ("&Lambda;", "Λ"),
    ("&Mu;", "Μ"),
    ("&Nu;", "Ν"),
    ("&OElig;", "Œ"),
    ("&Omega;", "Ω"),
    ("&Omicron;", "Ο"),
    ("&Phi;", "Φ"),
    ("&Pi;", "Π"),
    ("&Prime;", "″"),
    ("&Psi;", "Ψ"),
    ("&Rho;", "Ρ"),
    ("&Scaron;", "Š"),
    ("&Sigma;", "Σ"),
    ("&Tau;", "Τ"),
    ("&Theta;", "Θ"),
    ("&Upsilon;", "Υ"),
    ("&Xi;", "Ξ"),
    ("&Yuml;", "Ÿ"),
    ("&Zeta;", "Ζ"),
    ("&alefsym;", "ℵ"),
    ("&alpha;", "α"),
    ("&and;", "∧"),
    ("&ang;", "∠"),
    ("&asymp;", "≈"),
    ("&bdquo;", "„"),
    ("&beta;", "β"),
    ("&bull;", "•"),
    ("&cap;", "∩"),
    ("&chi;", "χ"),
    ("&circ;", "ˆ"),
    ("&clubs;", "♣"),
    ("&cong;", "≅"),
    ("&crarr;", "↵"),
    ("&cup;", "∪"),
    ("&dArr;", "⇓"),
    ("&dagger;", "†"),
    ("&darr;", "↓"),
    ("&delta;", "δ"),
    ("&diams;", "♦"),
    ("&empty;", "∅"),
    ("&emsp;", " "),
    ("&ensp;", " "),
    ("&epsilon;", "ε"),
    ("&equiv;", "≡"),
    ("&eta;", "η"),
    ("&euro;", "€"),
    ("&exist;", "∃"),
    ("&fnof;", "ƒ"),
    ("&forall;", "∀"),
    ("&frasl;", "⁄"),
    ("&gamma;", "γ"),
    ("&ge;", "≥"),
    ("&hArr;", "⇔"),
    ("&harr;", "↔"),
    ("&hearts;", "♥"),
    ("&hellip;", "…"),
    ("&image;", "ℑ"),
    ("&infin;", "∞"),
    ("&int;", "∫"),
    ("&iota;", "ι"),
    ("&isin;", "∈"),
    ("&kappa;", "κ"),
    ("&lArr;", "⇐"),
    ("&lambda;", "λ"),
    ("&lang;", "〈"),
    ("&larr;", "←"),
    ("&lceil;", "⌈"),
    ("&ldquo;", "“"),
    ("&le;", "≤"),
    ("&lfloor;", "⌊"),
    ("&lowast;", "∗"),
    ("&loz;", "◊"),
    ("&lrm;", "‎"),
    ("&lsaquo;", "‹"),
    ("&lsquo;", "‘"),
    ("&mdash;", "—"),
    ("&minus;", "−"),
    ("&mu;", "μ"),
    ("&nabla;", "∇"),
    ("&ndash;", "–"),
    ("&ne;", "≠"),
    ("&ni;", "∋"),
    ("&notin;", "∉"),
    ("&nsub;", "⊄"),
    ("&nu;", "ν"),
    ("&oelig;", "œ"),
    ("&oline;", "‾"),
    ("&omega;", "ω"),
    ("&omicron;", "ο"),
    ("&oplus;", "⊕"),
    ("&or;", "∨"),
    ("&otimes;", "⊗"),
    ("&part;", "∂"),
    ("&permil;", "‰"),
    ("&perp;", "⊥"),
    ("&phi;", "φ"),
    ("&pi;", "π"),
    ("&piv;", "ϖ"),
    ("&prime;", "′"),
    ("&prod;", "∏"),
    ("&prop;", "∝"),
    ("&psi;", "ψ"),
    ("&rArr;", "⇒"),
    ("&radic;", "√"),
    ("&rang;", "〉"),
    ("&rarr;", "→"),
    ("&rceil;", "⌉"),
    ("&rdquo;", "”"),
    ("&real;", "ℜ"),
    ("&rfloor;", "⌋"),
    ("&rho;", "ρ"),
    ("&rlm;", "‏"),
    ("&rsaquo;", "›"),
    ("&rsquo;", "’"),
    ("&sbquo;", "‚"),
    ("&scaron;", "š"),
    ("&sdot;", "⋅"),
    ("&sigma;", "σ"),
    ("&sigmaf;", "ς"),
    ("&sim;", "∼"),
    ("&spades;", "♠"),
    ("&sub;", "⊂"),
    ("&sube;", "⊆"),
    ("&sum;", "∑"),
    ("&sup;", "⊃"),
    ("&supe;", "⊇"),
    ("&tau;", "τ"),
    ("&there4;", "∴"),
    ("&theta;", "θ"),
    ("&thetasym;", "ϑ"),
    ("&thinsp;", " "),
    ("&tilde;", "˜"),
    ("&trade;", "™"),
    ("&uArr;", "⇑"),
    ("&uarr;", "↑"),
    ("&upsih;", "ϒ"),
    ("&upsilon;", "υ"),
    ("&weierp;", "℘"),
    ("&xi;", "ξ"),
    ("&zeta;", "ζ"),
    ("&zwj;", "‍"),
    ("&zwnj;", "‌"),
];

fn lookup(rest: &[char], out: &mut String) -> usize {
    let max = LONGEST.min(rest.len());
    // 由长到短的贪婪匹配,与 LookupTranslator 一致
    for i in (SHORTEST..=max).rev() {
        let key: String = rest[..i].iter().collect();
        if let Some((_, v)) = HTML4.iter().find(|(k, _)| *k == key) {
            out.push_str(v);
            return i;
        }
    }
    0
}

/// NumericEntityUnescaper(默认选项 = semiColonRequired)
fn numeric(rest: &[char], out: &mut String) -> usize {
    let seq_end = rest.len();
    // Java 的 `index < seqEnd - 2`:&# 后至少还有一个字符
    if rest[0] != '&' || seq_end < 3 || rest[1] != '#' {
        return 0;
    }
    let mut start = 2usize;
    let is_hex = matches!(rest[start], 'x' | 'X');
    if is_hex {
        start += 1;
        if start == seq_end {
            return 0;
        }
    }
    let mut end = start;
    while end < seq_end && rest[end].is_ascii_hexdigit() {
        end += 1;
    }
    let semi_next = end != seq_end && rest[end] == ';';
    if !semi_next {
        return 0; // semiColonRequired
    }
    let digits: String = rest[start..end].iter().collect();
    let radix = if is_hex { 16 } else { 10 };
    // Java Integer.parseInt 溢出也抛 NumberFormatException → 不翻译
    let Ok(value) = i32::from_str_radix(&digits, radix) else {
        return 0;
    };
    // 代理区与越界码点在 Java 侧会写出无效 UTF-16;Rust 无法表示,记 U+FFFD
    out.push(char::from_u32(value as u32).unwrap_or('\u{FFFD}'));
    2 + end - start + usize::from(is_hex) + usize::from(semi_next)
}

/// `StringEscapeUtils.unescapeHtml4`
pub fn unescape_html4(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut pos = 0usize;
    while pos < chars.len() {
        let rest = &chars[pos..];
        let mut consumed = lookup(rest, &mut out);
        if consumed == 0 {
            consumed = numeric(rest, &mut out);
        }
        if consumed == 0 {
            out.push(chars[pos]);
            pos += 1;
        } else {
            pos += consumed;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::unescape_html4;

    #[test]
    fn basic_and_extended() {
        assert_eq!(unescape_html4("a&amp;b&nbsp;c"), "a&b\u{a0}c");
        assert_eq!(unescape_html4("&notin;/&not;"), "\u{2209}/\u{ac}");
        assert_eq!(unescape_html4("&apos;"), "&apos;"); // HTML4 表无 apos
        assert_eq!(unescape_html4("&amp"), "&amp"); // 必须带分号
    }

    #[test]
    fn numeric_entities() {
        assert_eq!(unescape_html4("&#65;&#x42;"), "AB");
        assert_eq!(unescape_html4("&#12a;"), "&#12a;"); // 十进制解析失败原样保留
        assert_eq!(unescape_html4("&#65"), "&#65"); // 缺分号
        assert_eq!(unescape_html4("&#128512;"), "\u{1f600}");
    }
}
