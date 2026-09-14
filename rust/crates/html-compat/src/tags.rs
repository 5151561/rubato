//! jsoup 1.16.2 Tag 注册表(parser/Tag.java 静态表逐字迁移)。
//! 未知标签:非块级、非空、不保留空白(与 jsoup 一致)。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TagFlags {
    pub is_block: bool,
    pub format_as_block: bool,
    pub empty: bool,
    pub preserve_whitespace: bool,
}

#[rustfmt::skip] // 手排的分列表:rustfmt 会拆成一行一个
const BLOCK_TAGS: &[&str] = &[
    "html", "head", "body", "frameset", "script", "noscript", "style", "meta", "link", "title",
    "frame", "noframes", "section", "nav", "aside", "hgroup", "header", "footer", "p", "h1", "h2",
    "h3", "h4", "h5", "h6", "ul", "ol", "pre", "div", "blockquote", "hr", "address", "figure",
    "figcaption", "form", "fieldset", "ins", "del", "dl", "dt", "dd", "li", "table", "caption",
    "thead", "tfoot", "tbody", "colgroup", "col", "tr", "th", "td", "video", "audio", "canvas",
    "details", "menu", "plaintext", "template", "article", "main", "svg", "math", "center",
    "template", "dir", "applet", "marquee", "listing",
];

#[rustfmt::skip] // 手排的分列表:rustfmt 会拆成一行一个
const INLINE_TAGS: &[&str] = &[
    "object", "base", "font", "tt", "i", "b", "u", "big", "small", "em", "strong", "dfn", "code",
    "samp", "kbd", "var", "cite", "abbr", "time", "acronym", "mark", "ruby", "rt", "rp", "rtc",
    "a", "img", "br", "wbr", "map", "q", "sub", "sup", "bdo", "iframe", "embed", "span", "input",
    "select", "textarea", "label", "button", "optgroup", "option", "legend", "datalist", "keygen",
    "output", "progress", "meter", "area", "param", "source", "track", "summary", "command",
    "device", "basefont", "bgsound", "menuitem", "data", "bdi", "s", "strike", "nobr", "rb",
];
// 注意 **不含** text / mi / mo / msup / mn / mtext:jsoup 1.16.2 的 Tag 表里
// 没有它们,按未知标签走(formatAsBlock=true,序列化会缩进)。差分实测:
// `<div><text>x</text></div>` 裁判给 `<div>\n <text>\n  x\n </text>\n</div>`。
// 这条是 xpath 套照出来的 —— following-sibling 轴把文本节点包成 `Element("text")`。

#[rustfmt::skip] // 手排的分列表:rustfmt 会拆成一行一个
const EMPTY_TAGS: &[&str] = &[
    "meta", "link", "base", "frame", "img", "br", "wbr", "embed", "hr", "input", "keygen", "col",
    "command", "device", "area", "basefont", "bgsound", "menuitem", "param", "source", "track",
];

#[rustfmt::skip] // 手排的分列表:rustfmt 会拆成一行一个
const FORMAT_AS_INLINE_TAGS: &[&str] = &[
    "title", "a", "p", "h1", "h2", "h3", "h4", "h5", "h6", "pre", "address", "li", "th", "td",
    "script", "style", "ins", "del", "s",
];

const PRESERVE_WHITESPACE_TAGS: &[&str] = &["pre", "plaintext", "title", "textarea"];

/// script/style 的文本子节点在 jsoup 中是 DataNode(不进 text())
pub const DATA_TAGS: &[&str] = &["script", "style"];

/// 五张表的**逐字面量**结论,进程内只算一次。
///
/// 为什么要这张表:`tag_flags` 在选择器匹配与文本/序列化走查的最内层,而原来
/// 每次调用要对五个 `&[&str]` 各做一遍线性 `contains` —— 一个节点上百次字符串
/// 比较。表的内容与下面 [`compute_flags`] 逐位一致(键取五张表的并集,不在并集
/// 里的就是「解析期新建的未知标签」那一档),换的只是查法。
static FLAGS: std::sync::LazyLock<std::collections::HashMap<&'static str, TagFlags>> =
    std::sync::LazyLock::new(|| {
        BLOCK_TAGS
            .iter()
            .chain(INLINE_TAGS)
            .chain(EMPTY_TAGS)
            .chain(PRESERVE_WHITESPACE_TAGS)
            .map(|&n| (n, compute_flags(n)))
            .collect()
    });

pub fn tag_flags(name: &str) -> TagFlags {
    // 未命中 = 不在任何一张表里 = 未知标签,`compute_flags` 对它的结论是常量
    FLAGS.get(name).copied().unwrap_or(TagFlags {
        is_block: false,
        format_as_block: true,
        empty: false,
        preserve_whitespace: false,
    })
}

fn compute_flags(name: &str) -> TagFlags {
    // jsoup 三类:注册的块级(isBlock=true,formatAsBlock 视 formatAsInline 表);
    // 注册的行内(两者都 false);解析期新建的未知标签(Tag.valueOf:
    // isBlock=false,但 formatAsBlock 保持字段默认 true——组合很怪但即是裁判行为)
    let (is_block, format_as_block) = if BLOCK_TAGS.contains(&name) {
        (true, !FORMAT_AS_INLINE_TAGS.contains(&name))
    } else if INLINE_TAGS.contains(&name) {
        (false, false)
    } else {
        (false, true)
    };
    TagFlags {
        is_block,
        format_as_block,
        empty: EMPTY_TAGS.contains(&name),
        preserve_whitespace: PRESERVE_WHITESPACE_TAGS.contains(&name),
    }
}

/// HTML5 布尔属性(Attribute.java booleanAttributes,已排序)
#[rustfmt::skip] // 手排的分列表:rustfmt 会拆成一行一个
pub const BOOLEAN_ATTRIBUTES: &[&str] = &[
    "allowfullscreen", "async", "autofocus", "checked", "compact", "declare", "default", "defer",
    "disabled", "formnovalidate", "hidden", "inert", "ismap", "itemscope", "multiple", "muted",
    "nohref", "noresize", "noshade", "novalidate", "nowrap", "open", "readonly", "required",
    "reversed", "seamless", "selected", "sortable", "truespeed", "typemustmatch",
];

#[cfg(test)]
mod flags_tests {
    use super::*;

    /// 查表与逐张表现算必须**逐位一致** —— 这是上面那张缓存表唯一的正当性。
    #[test]
    fn table_matches_compute() {
        let mut names: Vec<&str> = BLOCK_TAGS
            .iter()
            .chain(INLINE_TAGS)
            .chain(EMPTY_TAGS)
            .chain(PRESERVE_WHITESPACE_TAGS)
            .chain(FORMAT_AS_INLINE_TAGS)
            .chain(DATA_TAGS)
            .copied()
            .collect();
        // 未注册的几个:未知标签那一档也要对上
        names.extend(["#root", "custom-x", "mycomp", ""]);
        for n in names {
            assert_eq!(tag_flags(n), compute_flags(n), "标签 {n} 的位对不上");
        }
    }
}
