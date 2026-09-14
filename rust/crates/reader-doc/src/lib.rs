//! 阅读正文的**文档模型**:把一章正文切成块,并把 UTF-16 口径钉死。
//!
//! 计划书 `docs/reader-layout-plan.md` §5.1 / §5.5。这一层只回答两个问题:
//!
//! 1. 这一章有哪些块、每块覆盖正文的哪一段(**UTF-16 code unit**);
//! 2. 块内的 render 下标怎么换回持久化用的 source 下标。
//!
//! 不依赖 Flutter、FFI、数据库与网络 —— 单测直接喂字符串。
//!
//! # 为什么是 UTF-16
//!
//! `durChapterPos` 与裁判 `Book.durChapterPos` 同口径(计划书 §3.1),而那边是
//! Java 的 `String` 下标,即 UTF-16 code unit。Dart 的 `String` 下标天然也是它。
//! **Rust 这一侧禁止把它当 UTF-8 byte index 用** —— 所以 [`SourceUtf16`] 与
//! [`RenderUtf16`] 是两个不提供 `From`/`as` 的 newtype:混用要在编译期失败,
//! 不是等到某本带 emoji 的书把进度落错地方才发现。
//!
//! # M1 的 SourceMap 是恒等的
//!
//! 计划书 §5.1 里的 `SourceMap` 现在**没有单开结构**:M1 的 render 文本就是
//! source 文本的一段切片,映射退化成「块首位移」一个数,记在 [`Block`] 上。
//! M3 往 render 文本里插分隔符表达禁则时(§7.2),真映射从
//! [`Block::to_source`] 这里长出来 —— 调用方那时不用改。
//! 提前造一个恒等的映射结构只会让人以为它已经在做事。

#![forbid(unsafe_code)]

mod img;
pub mod wire;

/// 文档模型的版本位。跨 FFI 的每一份结果都带上它;
/// 对不上就整份拒绝,不做「按字段猜」的兼容(计划书 §5)。
///
/// **v4**(M3d):块表的**位一个没动**,跟着页表一起升号(页表的摆放记录里
/// v1 那个 `reserved` 换成了 `line_gap`)。三段共用一个号。
///
/// **v3**(M2b):块表多了图片那一种,记录里跟着多了 `src_start` / `src_end`。
pub const SCHEMA_VERSION: u16 = 4;

/// 段首缩进字符(全角空格 U+3000)。**正文里的真实字符**,不是样式:
/// `html-format` 已按真身 `HtmlFormatter.format` 插进正文
/// (`rust/crates/html-format/src/lib.rs`),计入 `durChapterPos`。
/// 这里只**数**有几个,不剥离 —— 剥离就是破坏性口径变更(计划书 §5.1)。
pub const INDENT_CHAR: char = '\u{3000}';

/// 持久化用的正文下标:本章正文的 UTF-16 code unit 位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceUtf16(u32);

/// 送去整形/测量的那份文本里的 UTF-16 下标。M1 里它等于块内偏移。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderUtf16(u32);

impl SourceUtf16 {
    pub const ZERO: Self = Self(0);

    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl RenderUtf16 {
    pub const ZERO: Self = Self(0);

    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// 建文档时能出的错。**一条都不静默兜底**:章太长就拒绝整章,
/// 不截断成一半正文再让用户去猜为什么后半章不见了(计划书 §5.5)。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocError {
    /// 本章 UTF-16 长度超过 `u32::MAX`
    TooLong { utf16_len: u64 },
}

impl std::fmt::Display for DocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocError::TooLong { utf16_len } => {
                write!(f, "章节正文过长:{utf16_len} 个 UTF-16 code unit,上限 {}", u32::MAX)
            }
        }
    }
}

impl std::error::Error for DocError {}

/// 标题的归一化:去掉首尾空白,换行一律收成空格 —— 标题恒为**一个块**。
///
/// 空白标题归一化成空串,调用方据此判「这一章没有标题」。
pub fn normalize_title(title: &str) -> String {
    title.trim().replace(['\n', '\r'], " ")
}

/// 标题接进正文之后的那一份文本。
///
/// **只在这里拼一次**。文档模型按它切块、Dart 按块表切它的片,两者必须是
/// 逐字节同一份;拼两遍(一遍给切块、一遍给 Dart)就是留一处会各自漂的差异。
pub fn document_text(title: &str, body: &str) -> String {
    let title = normalize_title(title);
    if title.is_empty() { body.to_string() } else { format!("{title}\n{body}") }
}

/// 块的种类。**决定用哪一套样式测量**(计划书 M2 的「标题/正文样式分离」)。
///
/// 判种类的是 [`ChapterDocument`],不是排版侧:块序与种类都由切块那一遍定死,
/// 换字号不会让一个块从标题变成段落,也不会让一张图片变回一段文字。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BlockKind {
    Paragraph,
    Title,
    /// 块级图片。**render 文本为空**(`text_end == source_start`),
    /// 它的高度由 Dart 那一侧按图片自身尺寸与视口算(计划书 §5.1)
    Image,
}

impl BlockKind {
    /// 线格式里的取值。Dart 那一半读到不认识的值必须整份拒绝。
    pub const fn wire(self) -> u32 {
        match self {
            BlockKind::Paragraph => 0,
            BlockKind::Title => 1,
            BlockKind::Image => 2,
        }
    }
}

/// 一个正文块:段落、标题或图片。
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Block {
    /// 块在本章里的稳定 ID。**就是块序号**:同一份正文切出来的块序固定,
    /// 而正文一变 `revision` 就变、Dart 侧整批缓存作废,所以不需要内容哈希。
    pub id: u64,
    pub kind: BlockKind,
    /// 块首字(含缩进字符)在本章正文里的位置
    pub source_start: SourceUtf16,
    /// 块正文的末尾,**不含**块之间的换行符
    pub text_end: SourceUtf16,
    /// 下一块的 `source_start`;末块 = `source_utf16_len`。
    /// 块之间的换行符算在**前**一块头上,这样全章被无缝分割、没有空隙。
    pub source_end: SourceUtf16,
    /// 行首算作缩进的字符数(前导 [`INDENT_CHAR`] 的个数)。
    /// M1 不用它,M3 的两端对齐/悬挂要按它排除首行缩进宽度。图片块恒为 0。
    pub indent_len: u32,
    /// 图片 URL(含 `,{option}`)在本章正文里的区间。
    /// **只对 [`BlockKind::Image`] 有意义**,其余块两端相等。
    ///
    /// 为什么记这一对而不是让 Dart 自己从 `<img src="...">` 里抠:那是**第二个
    /// 解析器**,而两个解析器迟早各自漂(带 `,{"width":"50%"}` 的 src 里有引号,
    /// 抠法一不小心就断在中间)。这一侧本来就要认标签才切得出块,顺手把区间记下来。
    pub src_start: SourceUtf16,
    pub src_end: SourceUtf16,
}

impl Block {
    /// 块内 render 下标 → 持久化 source 下标。M1 是加一个块首位移;
    /// 越界返回 `None`(不 clamp:调用方拿错行号应当当场发现)。
    pub fn to_source(&self, render: RenderUtf16) -> Option<SourceUtf16> {
        let s = self.source_start.get().checked_add(render.get())?;
        (s <= self.text_end.get()).then_some(SourceUtf16(s))
    }

    /// source 下标 → 块内 render 下标。落在本块之外返回 `None`。
    pub fn to_render(&self, source: SourceUtf16) -> Option<RenderUtf16> {
        let s = source.get();
        (s >= self.source_start.get() && s <= self.text_end.get())
            .then(|| RenderUtf16(s - self.source_start.get()))
    }

    /// 本块正文的 UTF-16 长度(不含块间换行)。图片块恒为 0
    pub fn render_len(&self) -> u32 {
        self.text_end.get() - self.source_start.get()
    }

    pub fn is_image(&self) -> bool {
        self.kind == BlockKind::Image
    }
}

/// 一章的文档模型。
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ChapterDocument {
    pub schema_version: u16,
    /// 正文内容指纹。正文没变就不变 —— Dart 侧拿它当整批 Paragraph 缓存的前缀键
    pub revision: u64,
    /// 诊断用的人可读标识(`bookUrl#index`),不参与任何判等
    pub chapter_id: String,
    pub source_utf16_len: u32,
    pub blocks: Vec<Block>,
}

impl ChapterDocument {
    /// 切块(**无标题**)。规则只有一条:**按 `\n` 切**。
    ///
    /// 正文是 `HtmlFormatter.format` 的产物,段落之间就是单个 `\n`
    /// (`INDENT1` 把 `\s*\n+\s*` 收成了 `\n` + 缩进),所以不需要更聪明的切法。
    /// 空行照留 —— 它是正文里真实存在的一段,吞掉就对不上 `durChapterPos`。
    ///
    /// 空正文切出**一个空块**,不是零个块:计划书 §5.5 要求空章是「一页空页」,
    /// 零个块会让分页器连一页都排不出来。
    pub fn build(chapter_id: impl Into<String>, text: &str) -> Result<Self, DocError> {
        Self::split(chapter_id, text, 0)
    }

    /// 切块,并把**章节标题接在正文最前面**当第一个块。
    ///
    /// # 为什么标题是正文里的真字符,不是虚拟块
    ///
    /// 两条理由,一条对内一条对外:
    ///
    /// 1. **虚拟块会当场违反分页不变量**。虚拟块的 source 区间是空的
    ///    (`[0, 0)`),而计划书 §5.5 要求「任何非空页至少推进 1 个 code unit」。
    ///    标题独占一页时,那一页的区间就是 `[0, 0)`,后一页也从 0 起 ——
    ///    装页器会把它并进去(`merged_pages` 加一),标题页就此消失;
    /// 2. **裁判的 `durChapterPos` 本来就含标题**。`ReadBook.kt:1725,1739`:
    ///    `bodyPosition = durChapterPos - titleLength`,标题是排版内容的第一块、
    ///    它的长度计进进度。契约(计划书 §3.1)因此是**更贴近**了,不是漂了。
    ///
    /// 代价说清楚:M1 存下的进度是**不含标题**的正文下标,读回来会往前偏
    /// 一个标题的长度(十几个 code unit,不到一行)。这是一次性的,
    /// 不随设置反复漂 —— Rubato **不做**「隐藏标题」开关,正是为了不引入
    /// 裁判那边 `resolveHighlightChapterPosition` 那套补偿(标题一藏,
    /// 全章 offset 平移)。要做那个开关时,补偿是它自己的账。
    ///
    /// 标题里的换行一律收成空格:标题恒为**一个块**,省掉「标题占几块」
    /// 这个要在两侧同步的数。标题为空白时退回 [`Self::build`]。
    pub fn build_with_title(
        chapter_id: impl Into<String>,
        title: &str,
        body: &str,
    ) -> Result<Self, DocError> {
        let has_title = !normalize_title(title).is_empty();
        Self::split(chapter_id, &document_text(title, body), usize::from(has_title))
    }

    /// 切块的本体。`title_blocks == 1` 时**第一行**切出来的块记成
    /// [`BlockKind::Title`](标题恒为一行,见 [`normalize_title`])。
    ///
    /// 两步:先在**字节空间**切出片(切块的判断全是切片与空白判定,字节最直接),
    /// 再一趟把边界换成 UTF-16 下标。两件事分开,省掉「一边数 code unit
    /// 一边认标签」那种两个游标互相绊的写法。
    fn split(
        chapter_id: impl Into<String>,
        text: &str,
        title_blocks: usize,
    ) -> Result<Self, DocError> {
        let utf16_len: u64 = text.chars().map(|c| c.len_utf16() as u64).sum();
        if utf16_len > u32::MAX as u64 {
            return Err(DocError::TooLong { utf16_len });
        }

        let pieces = cut(text, title_blocks > 0);
        let mut cursor = Utf16Cursor::new(text);
        let mut blocks = Vec::with_capacity(pieces.len());
        for (i, p) in pieces.iter().enumerate() {
            let source_start = SourceUtf16(cursor.at(p.start));
            let text_end = SourceUtf16(cursor.at(p.text_end));
            let (src_start, src_end) = match p.src {
                Some((a, b)) => (SourceUtf16(cursor.at(a)), SourceUtf16(cursor.at(b))),
                None => (text_end, text_end),
            };
            blocks.push(Block {
                id: i as u64,
                kind: p.kind,
                source_start,
                text_end,
                source_end: SourceUtf16(cursor.at(p.end)),
                indent_len: p.indent_len,
                src_start,
                src_end,
            });
        }

        let source_utf16_len = cursor.at(text.len());
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            revision: fnv1a64(text.as_bytes()),
            chapter_id: chapter_id.into(),
            source_utf16_len,
            blocks,
        })
    }

    /// 落在哪一块。`offset == source_utf16_len` 合法(章末锚点),给末块。
    pub fn block_at(&self, offset: SourceUtf16) -> usize {
        match self.blocks.binary_search_by(|b| {
            if offset < b.source_start {
                std::cmp::Ordering::Greater
            } else if offset >= b.source_end {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        }) {
            Ok(i) => i,
            // 只可能是 offset == source_utf16_len(末块的 source_end)
            Err(_) => self.blocks.len() - 1,
        }
    }
}

/// 切块的中间产物:**字节空间**里的一片。字段与 [`Block`] 同名同义。
struct Piece {
    kind: BlockKind,
    start: usize,
    text_end: usize,
    end: usize,
    indent_len: u32,
    /// 图片 src 的字节区间
    src: Option<(usize, usize)>,
}

impl Piece {
    fn text(kind: BlockKind, text: &str, start: usize, text_end: usize, end: usize) -> Self {
        let indent_len =
            text[start..text_end].chars().take_while(|&c| c == INDENT_CHAR).count() as u32;
        Piece { kind, start, text_end, end, indent_len, src: None }
    }
}

/// 把正文切成片。规则两条:
///
/// 1. **按 `\n` 切**。正文是 `HtmlFormatter.format` 的产物,段落之间就是单个 `\n`
///    (`INDENT1` 把 `\s*\n+\s*` 收成了 `\n` + 缩进),不需要更聪明的切法。
///    空行照留 —— 它是正文里真实存在的一段,吞掉就对不上 `durChapterPos`;
/// 2. **行内的 `<img ...>` 各自成一片**(计划书 §5.1 的 `ImageBlock` 契约:
///    source range 覆盖整个标签、render 文本为空)。
///
/// # 图片两边的空白并进图片,不单独成块
///
/// `<p><img src="x"></p>` 经 `format_keep_img` 出来是 `　　<img src="x">` ——
/// 标签前面挂着段首缩进。把那两个全角空格切成独立的段落块,屏幕上就是图片
/// **上方多一条空行**。所以:图片与前一片之间**全是空白**的那一截并进图片块的
/// source 区间(它的 render 文本仍是空的,所以不显示),行尾剩下的空白同理。
///
/// 裁判那边是同一个处置:`TextChapterLayout.kt:504` 的
/// `if (textBefore.isNotBlank())` —— 空白的前缀直接丢掉不排。
/// 差别只在它是「丢掉」,这里是「并进图片的 source 区间」:块必须无缝覆盖全章,
/// 丢掉就出洞了(计划书 §5.5)。
fn cut(text: &str, title_line: bool) -> Vec<Piece> {
    let tags = img::scan(text);
    let mut out: Vec<Piece> = Vec::new();
    let mut tag_at = 0usize;
    let mut line_start = 0usize;
    let mut first_line = true;

    loop {
        let line_end = text[line_start..].find('\n').map(|i| line_start + i).unwrap_or(text.len());
        let nl = usize::from(line_end < text.len());
        let kind = if first_line && title_line { BlockKind::Title } else { BlockKind::Paragraph };

        let from = tag_at;
        while tag_at < tags.len() && tags[tag_at].end <= line_end {
            tag_at += 1;
        }
        let line_tags = &tags[from..tag_at];

        if line_tags.is_empty() {
            out.push(Piece::text(kind, text, line_start, line_end, line_end + nl));
        } else {
            let mut run = line_start;
            for t in line_tags {
                // 空白前缀并进图片;非空白的前缀是它自己的一段正文
                let start = if text[run..t.start].trim().is_empty() {
                    run
                } else {
                    out.push(Piece::text(kind, text, run, t.start, t.start));
                    t.start
                };
                out.push(Piece {
                    kind: BlockKind::Image,
                    start,
                    // **render 文本为空**:图片块不参与整形与测量
                    text_end: start,
                    end: t.end,
                    indent_len: 0,
                    src: Some((t.src_start, t.src_end)),
                });
                run = t.end;
            }
            if text[run..line_end].trim().is_empty() {
                // 行尾的空白(以及换行符)并进最后那张图片
                out.last_mut().expect("行内有图片").end = line_end + nl;
            } else {
                out.push(Piece::text(kind, text, run, line_end, line_end + nl));
            }
        }

        first_line = false;
        if nl == 0 {
            break;
        }
        line_start = line_end + 1;
    }
    out
}

/// 字节下标 → UTF-16 下标。请求必须**单调不减**(切块的边界天然如此),
/// 所以一趟扫完整章,不建索引表。
struct Utf16Cursor<'a> {
    chars: std::str::Chars<'a>,
    byte: usize,
    utf16: u32,
}

impl<'a> Utf16Cursor<'a> {
    fn new(text: &'a str) -> Self {
        Self { chars: text.chars(), byte: 0, utf16: 0 }
    }

    fn at(&mut self, byte: usize) -> u32 {
        debug_assert!(byte >= self.byte, "字节下标必须单调不减");
        while self.byte < byte {
            let c = self.chars.next().expect("边界落在正文内");
            self.byte += c.len_utf8();
            self.utf16 += c.len_utf16() as u32;
        }
        self.utf16
    }
}

/// FNV-1a 64。只用来当内容指纹(判「正文变没变」),不做安全用途,
/// 为它拉一个哈希库不值得。
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16_len(s: &str) -> u32 {
        s.encode_utf16().count() as u32
    }

    /// 全章被块无缝分割:没有空隙、没有重叠、覆盖到 `source_utf16_len`
    fn assert_seamless(doc: &ChapterDocument) {
        assert_eq!(doc.blocks[0].source_start, SourceUtf16::ZERO);
        for w in doc.blocks.windows(2) {
            assert_eq!(w[0].source_end, w[1].source_start, "块之间有空隙或重叠");
            assert!(w[0].source_start <= w[0].text_end);
            assert!(w[0].text_end <= w[0].source_end);
        }
        let last = doc.blocks.last().expect("至少一块");
        assert_eq!(last.source_end.get(), doc.source_utf16_len);
    }

    #[test]
    fn 空正文切出一个空块() {
        let doc = ChapterDocument::build("t", "").expect("build");
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(doc.source_utf16_len, 0);
        assert_eq!(doc.blocks[0].render_len(), 0);
        assert_seamless(&doc);
    }

    #[test]
    fn 按换行切块且缩进算正文字符() {
        let text = "　　第一段。\n　　第二段更长一点。";
        let doc = ChapterDocument::build("t", text).expect("build");
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(doc.blocks[0].indent_len, 2);
        assert_eq!(doc.blocks[1].indent_len, 2);
        // 缩进字符没有被剥离:块首就是 `　`
        assert_eq!(doc.blocks[0].source_start, SourceUtf16::ZERO);
        assert_eq!(doc.blocks[0].render_len(), utf16_len("　　第一段。"));
        // 换行算在前一块头上
        assert_eq!(doc.blocks[0].source_end.get(), doc.blocks[0].text_end.get() + 1);
        assert_eq!(doc.source_utf16_len, utf16_len(text));
        assert_seamless(&doc);
    }

    #[test]
    fn 空行照留() {
        let doc = ChapterDocument::build("t", "甲\n\n乙").expect("build");
        assert_eq!(doc.blocks.len(), 3);
        assert_eq!(doc.blocks[1].render_len(), 0);
        assert_seamless(&doc);
    }

    #[test]
    fn 末尾换行也是一个空块() {
        let doc = ChapterDocument::build("t", "甲\n").expect("build");
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(doc.blocks[1].render_len(), 0);
        assert_seamless(&doc);
    }

    #[test]
    fn emoji_按两个_code_unit_记() {
        // 😀 = U+1F600,UTF-16 是一对代理项 = 2 个 code unit,UTF-8 是 4 字节。
        // 这条测的就是「别把 UTF-16 下标当 byte index」。
        let text = "😀甲\n乙";
        let doc = ChapterDocument::build("t", text).expect("build");
        assert_eq!(doc.blocks[0].render_len(), 3);
        assert_eq!(doc.blocks[1].source_start.get(), 4);
        assert_eq!(doc.source_utf16_len, utf16_len(text));
        assert_seamless(&doc);
    }

    #[test]
    fn render_与_source_只经映射互转() {
        let doc = ChapterDocument::build("t", "甲乙\n丙丁").expect("build");
        let b = &doc.blocks[1];
        assert_eq!(b.to_source(RenderUtf16::new(1)), Some(SourceUtf16::new(4)));
        // 块末合法(章末/段末锚点),再往后非法
        assert_eq!(b.to_source(RenderUtf16::new(2)), Some(SourceUtf16::new(5)));
        assert_eq!(b.to_source(RenderUtf16::new(3)), None);
        assert_eq!(b.to_render(SourceUtf16::new(4)), Some(RenderUtf16::new(1)));
        assert_eq!(b.to_render(SourceUtf16::new(2)), None);
    }

    #[test]
    fn block_at_覆盖到章末锚点() {
        let doc = ChapterDocument::build("t", "甲\n乙\n丙").expect("build");
        assert_eq!(doc.block_at(SourceUtf16::new(0)), 0);
        assert_eq!(doc.block_at(SourceUtf16::new(1)), 0); // 换行归前一块
        assert_eq!(doc.block_at(SourceUtf16::new(2)), 1);
        assert_eq!(doc.block_at(SourceUtf16::new(4)), 2);
        assert_eq!(doc.block_at(SourceUtf16::new(doc.source_utf16_len)), 2);
    }

    #[test]
    fn 标题成为第一个块且计进正文长度() {
        let doc = ChapterDocument::build_with_title("t", "第一章 开端", "　　甲乙。\n　　丙丁。").expect("doc");
        assert_eq!(doc.blocks.len(), 3);
        assert_eq!(doc.blocks[0].kind, BlockKind::Title);
        assert_eq!(doc.blocks[1].kind, BlockKind::Paragraph);
        assert_eq!(doc.blocks[2].kind, BlockKind::Paragraph);
        // 标题是正文里的真字符:块 0 覆盖它,块 1 从标题之后起
        assert_eq!(doc.blocks[0].render_len(), utf16_len("第一章 开端"));
        assert_eq!(doc.blocks[1].source_start.get(), utf16_len("第一章 开端") + 1);
        assert_eq!(doc.source_utf16_len, utf16_len("第一章 开端\n　　甲乙。\n　　丙丁。"));
        assert_seamless(&doc);
    }

    #[test]
    fn 标题里的换行收成空格_恒为一个块() {
        let doc = ChapterDocument::build_with_title("t", " 第一章\n开端 ", "甲").expect("doc");
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(doc.blocks[0].render_len(), utf16_len("第一章 开端"));
        assert_seamless(&doc);
    }

    #[test]
    fn 空白标题退回无标题() {
        let doc = ChapterDocument::build_with_title("t", "   ", "甲\n乙").expect("doc");
        assert_eq!(doc.blocks.len(), 2);
        assert!(doc.blocks.iter().all(|b| b.kind == BlockKind::Paragraph));
        // 与没有标题那条路逐位相同
        let plain = ChapterDocument::build("t", "甲\n乙").expect("plain");
        assert_eq!(doc.revision, plain.revision);
        assert_eq!(doc.source_utf16_len, plain.source_utf16_len);
    }

    /// 标题为空的章也得有块:标题空 + 正文空 = 一个空段落块,不是零块
    #[test]
    fn 标题与正文都空仍是一个块() {
        let doc = ChapterDocument::build_with_title("t", "", "").expect("doc");
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(doc.source_utf16_len, 0);
        assert_seamless(&doc);
    }

    #[test]
    fn 正文没变则_revision_不变() {
        let a = ChapterDocument::build("x", "甲乙丙").expect("a");
        let b = ChapterDocument::build("y", "甲乙丙").expect("b");
        let c = ChapterDocument::build("x", "甲乙丁").expect("c");
        assert_eq!(a.revision, b.revision);
        assert_ne!(a.revision, c.revision);
    }

    // ---------------- 块级图片(M2b) ----------------

    fn src_of(doc: &ChapterDocument, text: &str, i: usize) -> String {
        let b = &doc.blocks[i];
        let u: Vec<u16> = text.encode_utf16().collect();
        String::from_utf16(&u[b.src_start.get() as usize..b.src_end.get() as usize]).expect("utf16")
    }

    /// 独占一行的图片自成一块,**段首缩进并进它**——不然图片上方白出一条空行
    #[test]
    fn 块级图片自成一块且缩进并进它() {
        let text = "　　甲乙。\n　　<img src=\"http://a/1.jpg\">\n　　丙丁。";
        let doc = ChapterDocument::build("t", text).expect("doc");
        assert_eq!(doc.blocks.len(), 3, "缩进不该单独成块");
        assert_eq!(doc.blocks[1].kind, BlockKind::Image);
        // 图片块从缩进那两个全角空格起,一直盖到行尾的换行
        assert_eq!(doc.blocks[1].source_start.get(), utf16_len("　　甲乙。") + 1);
        assert_eq!(doc.blocks[1].render_len(), 0, "render 文本为空");
        assert_eq!(doc.blocks[1].indent_len, 0);
        assert_eq!(src_of(&doc, text, 1), "http://a/1.jpg");
        assert_eq!(doc.blocks[2].kind, BlockKind::Paragraph);
        assert_seamless(&doc);
    }

    /// 行内图片把一行切成三块。裁判也是这么处置的(逐个 img 切,前后文各自成行),
    /// 只是它把空白前缀**丢掉**,这里并进图片的 source 区间 —— 块必须无缝覆盖全章
    #[test]
    fn 行内图片把一行切成三块() {
        let text = "　　看这张<img src=\"a.jpg\">很好。";
        let doc = ChapterDocument::build("t", text).expect("doc");
        assert_eq!(doc.blocks.len(), 3);
        assert_eq!(
            [doc.blocks[0].kind, doc.blocks[1].kind, doc.blocks[2].kind],
            [BlockKind::Paragraph, BlockKind::Image, BlockKind::Paragraph]
        );
        assert_eq!(doc.blocks[0].render_len(), utf16_len("　　看这张"));
        assert_eq!(doc.blocks[0].indent_len, 2);
        assert_eq!(doc.blocks[2].render_len(), utf16_len("很好。"));
        assert_eq!(src_of(&doc, text, 1), "a.jpg");
        assert_seamless(&doc);
    }

    #[test]
    fn 一行里连着两张图是两块() {
        let text = "<img src=\"a\"><img src=\"b\">";
        let doc = ChapterDocument::build("t", text).expect("doc");
        assert_eq!(doc.blocks.len(), 2);
        assert!(doc.blocks.iter().all(|b| b.kind == BlockKind::Image));
        assert_eq!(src_of(&doc, text, 0), "a");
        assert_eq!(src_of(&doc, text, 1), "b");
        assert_seamless(&doc);
    }

    /// 整章只有一张图:一个块,而且它的 source 区间盖满全章 ——
    /// 分页那边据此排出一页(页首 0、末页盖到章末)
    #[test]
    fn 整章只有一张图() {
        let text = "<img src=\"a.jpg\">";
        let doc = ChapterDocument::build("t", text).expect("doc");
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(doc.blocks[0].kind, BlockKind::Image);
        assert_eq!(doc.blocks[0].source_start, SourceUtf16::ZERO);
        assert_eq!(doc.blocks[0].source_end.get(), doc.source_utf16_len);
        assert_seamless(&doc);
    }

    /// 带 `,{option}` 的 src 原样圈出来:option 怎么解是**取图那一侧**的事
    /// (`AnalyzeUrl` 自己会切),这一层不碰
    #[test]
    fn 带_option_的_src_原样交出去() {
        let text = "<img src=\"http://a/1.jpg,{\"width\":\"50%\"}\">";
        let doc = ChapterDocument::build("t", text).expect("doc");
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(src_of(&doc, text, 0), "http://a/1.jpg,{\"width\":\"50%\"}");
    }

    /// 认不出来的 `<img` 按**字面文本**走(与 M1 同款),不发明第三种块
    #[test]
    fn 认不出的图片标签按字面文本走() {
        let text = "　　<img alt=\"x\">还是文字";
        let doc = ChapterDocument::build("t", text).expect("doc");
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(doc.blocks[0].kind, BlockKind::Paragraph);
        assert_eq!(doc.blocks[0].render_len(), utf16_len(text));
        assert_seamless(&doc);
    }

    /// 标题那一行里的图片仍然是图片块,不会被标题那一位盖过去
    #[test]
    fn 图片不因为在标题行就变成标题() {
        let doc = ChapterDocument::build_with_title("t", "第一章", "<img src=\"a\">").expect("doc");
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(doc.blocks[0].kind, BlockKind::Title);
        assert_eq!(doc.blocks[1].kind, BlockKind::Image);
        assert_seamless(&doc);
    }

    /// UTF-16 口径:图片前面有 emoji 时 src 区间照样切得准
    #[test]
    fn 图片的_src_区间是_utf16_口径() {
        let text = "😀<img src=\"a.jpg\">";
        let doc = ChapterDocument::build("t", text).expect("doc");
        assert_eq!(doc.blocks[1].source_start.get(), 2, "emoji 占两个 code unit");
        assert_eq!(src_of(&doc, text, 1), "a.jpg");
        assert_seamless(&doc);
    }

    /// 随机正文里混进图片标签,不变量照样成立
    #[test]
    fn 带图片的随机正文都无缝覆盖() {
        let alphabet =
            ["甲", "\n", "　", "a", "😀", "<img src=\"x\">", "<img", "\">", "<img src=\"y,{\"w\":\"1\"}\">"];
        let mut seed: u64 = 0x51ED_2701_ABCD_1234;
        for _ in 0..500 {
            let mut text = String::new();
            let len = (seed >> 27) as usize % 30;
            for _ in 0..len {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                text.push_str(alphabet[(seed >> 33) as usize % alphabet.len()]);
            }
            let doc = ChapterDocument::build("t", &text).expect("build");
            assert_seamless(&doc);
            assert_eq!(doc.source_utf16_len, utf16_len(&text));
            for b in &doc.blocks {
                if b.is_image() {
                    assert_eq!(b.render_len(), 0, "图片块的 render 文本必须为空");
                    assert!(b.src_start >= b.source_start && b.src_end <= b.source_end);
                }
            }
        }
    }

    /// 随机正文的无缝性:换行、空行、emoji、缩进混着来
    #[test]
    fn 随机正文都无缝覆盖() {
        let alphabet = ['甲', '\n', '　', 'a', '😀', '。'];
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..500 {
            let mut text = String::new();
            let len = (seed >> 27) as usize % 40;
            for _ in 0..len {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                text.push(alphabet[(seed >> 33) as usize % alphabet.len()]);
            }
            let doc = ChapterDocument::build("t", &text).expect("build");
            assert_seamless(&doc);
            assert_eq!(doc.source_utf16_len, utf16_len(&text));
            // 每个合法 source 位置都能定位到块,且块内可换算
            for off in 0..=doc.source_utf16_len {
                let i = doc.block_at(SourceUtf16::new(off));
                let b = &doc.blocks[i];
                assert!(b.source_start.get() <= off);
                assert!(off <= b.source_end.get());
            }
        }
    }
}
