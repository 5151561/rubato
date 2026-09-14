//! 分页器:把「块 + 行度量」装成页。
//!
//! 计划书 §5.4 / §5.5。两条硬约束在这里兑现:
//!
//! - **行级 `Placement`**:页引用的是「某块的第 i..j 行」,不是「段落的某个矩形」。
//!   M3 改成 Rust 自己断行时,只换行的来源,不换这个形状;
//! - **无缝分割**:`page[0].start == 0`、`page[i].end == page[i+1].start`、
//!   `page[last].end == 已测量到的末尾`,且每页至少推进 1 个 code unit。
//!
//! 分页是**流式**的:页只依赖它前面的块。所以喂一段前缀进来算出的页,
//! 除最后一页外与喂全章算出来的**逐位相同** —— 首屏据此只量到锚点页够用的块
//! 就能先画出来(计划书 §6.1 第 2/4 步),不必等整章测量完。

use crate::measure::MeasureBatch;
use reader_doc::{ChapterDocument, RenderUtf16};

/// 页底对齐**撑得最多的那一档**:行与行之间加的空白,不超过本页平均行高的
/// 这个比例(计划书 M3d)。
///
/// 判据不是「这一页空了多少」而是「**每一道行距被撑开多少**」——同样空一行,
/// 二十行的页摊下去每道 5%(看不出来),三行的页摊下去每道 50%(当场露馅)。
/// 1/8 是「一眼看不出来」与「大部分页都能贴底」之间的取值:32 px 的行最多加 4 px。
///
/// 撑不到就**整页不撑**,不摊一半:摊一半的页照样不贴底,只是行距还变了。
pub const MAX_BOTTOM_GAP_RATIO: f32 = 0.125;

/// 孤行控制:一个块被拆到两页上时,**两边各至少留这么多行**(计划书 M3b)。
///
/// 2 是中文排版惯例(与西文的 orphans/widows 同源):段首一行孤零零留在页底、
/// 或段尾一行孤零零跑到次页顶,都是要避开的。
///
/// **不是设置**。它是排版惯例,不是口味 —— 开成滑块就得回答「1 是什么意思」,
/// 而 1 的意思是「关掉这条规则」。要它可调时再说,那时是 `ComposeParams`
/// 上已经开着的这个位。
pub const DEFAULT_MIN_SPLIT_LINES: u32 = 2;

/// 浮点比较的容差。行高与页高都是设备像素量级,`1e-3` 远小于一个像素,
/// 又能吃掉「行高累加」与「Paragraph 报的 top」之间的最后一位误差。
const EPS: f32 = 1e-3;

/// 一页里的一件东西。M1 只有一种 —— 但**从第一版就是枚举**:
/// M2 的图片、M4 的高亮进来时是加一个变体,不是改协议(计划书 §5.4)。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Placement {
    /// 某块连续的一段行。绘制方按
    /// `y = origin_y + line.top + (i - line_start) * line_gap` 逐行画 ——
    /// `origin_y` 是「块内 top=0 映射到的页内 y」,跨页续排时是负数。
    Lines {
        block_id: u64,
        line_start: u32,
        line_end: u32,
        origin_x: f32,
        origin_y: f32,
        /// **页底对齐**摊进来的额外行距(计划书 M3d)。恒为 0 时就是 M3c 之前的
        /// 摆放。它在**摆放**上而不是在行度量上:同一份行度量在不同的页上摊到的
        /// 数不一样(每页剩多少空白不同),所以它是「这一页怎么摆」的一部分
        line_gap: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PagePlan {
    pub page_index: u32,
    /// 本页首字在本章正文里的 UTF-16 下标 —— 就是落库的 `durChapterPos`
    pub source_start: u32,
    pub source_end: u32,
    pub placements: Vec<Placement>,
}

#[derive(Clone, Copy, Debug)]
pub struct ComposeParams {
    /// 可排版区域的高度(已扣掉页边距)
    pub viewport_height: f32,
    /// **滚动模式**:不断页,整章排成**一页**,`origin_y` 是章内绝对坐标
    /// (计划书 M2c)。
    ///
    /// 为什么是一个 bool 而不是「把 viewport_height 传成无穷大」:后者是个
    /// 魔法值 —— 校验那一句(`is_finite`)得为它开口子,而那句挡的正是
    /// 「Dart 那边算出了 NaN/Inf」。模式是模式,尺寸是尺寸。
    ///
    /// 滚动模式下 `viewport_height` **照样要合法**:装页不用它,但它是同一份
    /// 排版条件的一部分(Dart 拿它算图片占多高、算可见窗口),不该在这条路上
    /// 变成「随便传什么都行」。
    pub scroll: bool,
    /// 孤行控制的最小行数,见 [`DEFAULT_MIN_SPLIT_LINES`]。`<= 1` = 关掉这条规则
    /// (M3b 之前的行为),用例靠它对照「这条规则到底改了什么」。
    pub min_split_lines: u32,
    /// **页底对齐**(计划书 M3d):把「差一点点就满」的那截空白摊进行距,
    /// 让每一页的正文下缘对齐。见 [`bottom_align`]。
    pub bottom_align: bool,
}

/// 装页时撞上的**不该发生**的事。**非空即异常**,调用方必须记进日志:
/// 计划书 §5.5 不许静默兜底 —— 静默兜底会把「一页装不下一个字」
/// 藏成「正文被切碎」,查起来毫无线索。
///
/// 计划书原话是 `debug_assert!` + 上报诊断。这里**只上报,不 assert**:
/// debug_assert 会让这两条路在单测里根本走不到(一进去就 panic),
/// 而它们恰恰是最需要有用例盯着的两条。响也响了 —— 引擎那侧收到非零计数
/// 就打 warning(见 `engine::reader`),不比 assert 安静。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ComposeDiagnostics {
    /// 单行比整页还高,只能让它自己占一页(页会溢出)
    pub oversized_lines: u32,
    /// 相邻两页首字下标没有推进,后一页被并进前一页
    pub merged_pages: u32,
}

impl ComposeDiagnostics {
    pub fn is_clean(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Clone, Debug)]
pub struct ComposeResult {
    pub pages: Vec<PagePlan>,
    /// 这次装页吃进了多少块。`< doc.blocks.len()` 就是前缀装页
    pub measured_blocks: u32,
    /// 整章都测量到了吗。false 时**最后一页还会长**,不能拿它当末页用
    pub complete: bool,
    pub diagnostics: ComposeDiagnostics,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposeError {
    /// 视口宽高或行高退化。拒绝这次布局,调用方保留上一次结果(计划书 §5.5)
    BadViewport,
    /// 测量批次比文档还长,或块序对不上
    BlockMismatch { at: usize },
    /// 行的 render 区间超出了它所属块的正文长度
    LineOutOfRange { block: usize, line: usize },
    /// 某块的 `space_before` 是负数或非有限值。**不 clamp**:
    /// 静默改成 0 只会让「Dart 那边的样式算出了 NaN」变成「排版偶尔差一截」
    BadSpacing { block: usize },
}

impl std::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComposeError::BadViewport => write!(f, "视口高度非正,拒绝本次布局"),
            ComposeError::BlockMismatch { at } => write!(f, "第 {at} 块的测量与文档对不上"),
            ComposeError::LineOutOfRange { block, line } => {
                write!(f, "第 {block} 块第 {line} 行的 render 区间越出正文")
            }
            ComposeError::BadSpacing { block } => {
                write!(f, "第 {block} 块的块前间距不是非负有限值")
            }
        }
    }
}

impl std::error::Error for ComposeError {}

/// 一个块该怎么切(计划书 M3b 的孤行控制)。
enum Split {
    /// 本页收到第 `0` 行(不含)为止
    At(usize),
    /// 这一块整块推到下一页去
    MoveBlock,
}

/// 本页从第 `i` 行起、能装到第 `j` 行,那么**该**装到第几行。
///
/// 两条规则在这里,一条都不需要装页器认识「标题」或「段落」:
///
/// - **页尾孤行**(widow):切完剩下的行少于 `min` 时,把切点往回拉到 `len - min`
///   —— 段尾一行孤零零跑到次页顶是要避开的;
/// - **页首孤行**(orphan):这一块**刚在本页开头**(`i == 0`)却只留得下不到
///   `min` 行,整块下推。块是从上一页续排来的(`i > 0`)时这条不适用 ——
///   那几行不是段首,只要能推进一行就行。
///
/// 两条一起的效果:`min = 2` 时,**行数 ≤ 3 的段落永远不拆**(2 行拆不出
/// 2+2,3 行也拆不出),4 行的段落只能拆成 2+2。
///
/// 只看**本块自己**的行数,不看后面还有什么 —— 前缀装页因此照旧成立
/// (计划书 §6.1:喂前缀算出来的页,除最后一页外与喂全章逐位相同)。
fn plan_split(len: usize, i: usize, j: usize, min: u32) -> Split {
    if j >= len {
        return Split::At(j); // 整块放得下,不用拆
    }
    let min = min.max(1) as usize;
    let mut k = j;
    if len - k < min {
        k = len.saturating_sub(min);
    }
    let floor = if i == 0 { min } else { 1 };
    if k < i + floor { Split::MoveBlock } else { Split::At(k) }
}

/// 页底对齐:把这一页**差一点点就满**的那截空白,均匀摊进行与行之间。
///
/// 撑多少算多,看的是**每一道行距被撑开多少**,不是「这一页空了多少」——
/// 同样空一行,二十行的页摊下去每道 5%(看不出来),三行的页摊下去每道 50%
/// (当场露馅)。上限是 [`MAX_BOTTOM_GAP_RATIO`];超了就整页不撑,
/// 不摊一半(摊一半的页照样不贴底,只是行距还变了)。
/// 口径一句话:**能对齐的对齐,对不齐的老实短着**。
///
/// 末页不进这里(调用方挑的):它本来就短,而且分批测量时它还会往下长。
fn bottom_align(placements: &mut [Placement], batch: &MeasureBatch, vh: f32) {
    let mut n = 0usize;
    let mut ink = 0.0f32;
    let mut bottom = 0.0f32;
    for pl in placements.iter() {
        let Placement::Lines { block_id, line_start, line_end, origin_y, .. } = pl;
        let lines = &batch.blocks[*block_id as usize].lines;
        for k in *line_start..*line_end {
            let l = &lines[k as usize];
            n += 1;
            ink += l.height;
            bottom = bottom.max(origin_y + l.top + l.height);
        }
    }
    // 一行的页没有「行与行之间」可摊
    if n < 2 {
        return;
    }
    let leftover = vh - bottom;
    if leftover <= EPS {
        return;
    }
    let gap = leftover / (n - 1) as f32;
    // 每一道行距最多撑这么多。撑不到就整页不撑 —— 那一页是被孤行控制
    // 或者放不下的图片弄短的,硬摊只会把「短」变成「行距怪」
    if gap > ink / n as f32 * MAX_BOTTOM_GAP_RATIO + EPS {
        return;
    }
    let mut before = 0usize;
    for pl in placements.iter_mut() {
        let Placement::Lines { line_start, line_end, origin_y, line_gap, .. } = pl;
        *origin_y += before as f32 * gap;
        *line_gap = gap;
        before += (*line_end - *line_start) as usize;
    }
}

/// 装页。`batch` 可以只覆盖文档的**前缀**(分批测量),覆盖到哪算到哪。
///
/// [`ComposeParams::scroll`] 为真时不断页:整章排成一页,`origin_y` 是章内绝对
/// 坐标。前缀装页在滚动下同样成立 —— 那一页会随着测量往下长,前面的摆放不动。
pub fn compose(
    doc: &ChapterDocument,
    batch: &MeasureBatch,
    params: ComposeParams,
) -> Result<ComposeResult, ComposeError> {
    if params.viewport_height <= 0.0 || !params.viewport_height.is_finite() {
        return Err(ComposeError::BadViewport);
    }
    if batch.blocks.len() > doc.blocks.len() {
        return Err(ComposeError::BlockMismatch { at: doc.blocks.len() });
    }

    // 滚动模式 = 页高无穷:一行都断不了,整章落成一页。
    // 分页那条路一位没动 —— 「不断页」本来就是「页高够大」的极限情形
    let vh = if params.scroll { f32::INFINITY } else { params.viewport_height };
    let mut diagnostics = ComposeDiagnostics::default();
    // (页首 source 下标, 本页的摆放)
    let mut pages: Vec<(u32, Vec<Placement>)> = Vec::new();
    let mut cur: Vec<Placement> = Vec::new();
    let mut cur_start: Option<u32> = None;
    let mut y = 0.0f32;

    for (bi, bm) in batch.blocks.iter().enumerate() {
        let block = &doc.blocks[bi];
        if bm.block_id != block.id {
            return Err(ComposeError::BlockMismatch { at: bi });
        }
        if !bm.space_before.is_finite() || bm.space_before < 0.0 {
            return Err(ComposeError::BadSpacing { block: bi });
        }
        // **页首折叠**:这一块正好落在页顶时不留块前间距。少了这一条,
        // 每一页顶上都会空出一个段距,而那一截空白既不是排版意图、
        // 也会让每页少装一行
        if !cur.is_empty() {
            y += bm.space_before;
        }
        let lines = &bm.lines;
        let mut i = 0usize;
        while i < lines.len() {
            let origin_y = y - lines[i].top;
            // 本页从第 i 行起还能放到第几行
            let mut j = i;
            while j < lines.len() && origin_y + lines[j].top + lines[j].height <= vh + EPS {
                j += 1;
            }
            if j == i {
                if cur.is_empty() {
                    // 页还空着就放不下一行 => 这一行比整页还高。
                    // 强行放,不然永远推进不了;但**留下证据**,不装作正常。
                    diagnostics.oversized_lines += 1;
                    j = i + 1;
                } else {
                    // 换页从头量:origin_y 会跟着变
                    pages.push((cur_start.take().expect("非空页必有页首"), std::mem::take(&mut cur)));
                    y = 0.0;
                    continue;
                }
            } else if let Split::At(k) = plan_split(lines.len(), i, j, params.min_split_lines) {
                j = k;
            } else if !cur.is_empty() {
                // 整块下推。**只有页上已经有东西时做得到** —— 页还空着时下推
                // 没有去处,那时照原样切(视口矮到装不下 `min` 行就是这一种)。
                // 那不是「排版意图没兑现」,是做不到:所以不记诊断,
                // 诊断位留给真正的异常
                pages.push((cur_start.take().expect("非空页必有页首"), std::mem::take(&mut cur)));
                y = 0.0;
                continue;
            }
            if cur_start.is_none() {
                let src = block
                    .to_source(RenderUtf16::new(lines[i].render_start))
                    .ok_or(ComposeError::LineOutOfRange { block: bi, line: i })?;
                cur_start = Some(src.get());
            }
            // 末行也要校验一次:Dart 报了越界的 render_end 必须当场判死
            if block.to_source(RenderUtf16::new(lines[j - 1].render_end)).is_none() {
                return Err(ComposeError::LineOutOfRange { block: bi, line: j - 1 });
            }
            cur.push(Placement::Lines {
                block_id: block.id,
                line_start: i as u32,
                line_end: j as u32,
                origin_x: 0.0,
                origin_y,
                line_gap: 0.0,
            });
            y = origin_y + lines[j - 1].top + lines[j - 1].height;
            i = j;
            if i < lines.len() {
                pages.push((cur_start.take().expect("非空页必有页首"), std::mem::take(&mut cur)));
                y = 0.0;
            }
        }
    }
    if let Some(start) = cur_start.take() {
        pages.push((start, cur));
    }

    // 空章:一页空页,不是零页(计划书 §5.5)
    if pages.is_empty() {
        pages.push((0, Vec::new()));
    }

    // 后置不变量。这里**不**照抄旧实现的 `offset += end > 0 ? end : 1`:
    // 硬推 1 会让 durChapterPos 指到一个不是本页首字的位置,比溢出更难查。
    // 推进不了的页并进前一页 —— 覆盖与单调都还在,证据也留下了。
    if pages[0].0 != 0 {
        diagnostics.merged_pages += 1;
        pages[0].0 = 0;
    }
    let mut merged: Vec<(u32, Vec<Placement>)> = Vec::with_capacity(pages.len());
    for (start, placements) in pages {
        match merged.last_mut() {
            Some(prev) if start <= prev.0 => {
                diagnostics.merged_pages += 1;
                prev.1.extend(placements);
            }
            _ => merged.push((start, placements)),
        }
    }

    // 页底对齐(M3d)。**末页不对齐**:它本来就短,分批测量时还会往下长。
    // 滚动模式没有「页底」这回事
    if params.bottom_align && !params.scroll {
        let last = merged.len() - 1;
        for (i, (_, placements)) in merged.iter_mut().enumerate() {
            if i != last {
                bottom_align(placements, batch, params.viewport_height);
            }
        }
    }

    let complete = batch.blocks.len() == doc.blocks.len();
    let tail_end = if complete {
        doc.source_utf16_len
    } else {
        batch.blocks.len().checked_sub(1).map_or(0, |i| doc.blocks[i].source_end.get())
    };

    let last = merged.len() - 1;
    let pages = merged
        .iter()
        .enumerate()
        .map(|(i, (start, placements))| PagePlan {
            page_index: i as u32,
            source_start: *start,
            source_end: if i == last { tail_end } else { merged[i + 1].0 },
            placements: placements.clone(),
        })
        .collect();

    Ok(ComposeResult {
        pages,
        measured_blocks: batch.blocks.len() as u32,
        complete,
        diagnostics,
    })
}

/// 锚点落在哪一页:最后一个 `source_start <= offset` 的页。
/// 页表非空是 [`compose`] 的后置条件,所以这里不会返回越界值。
pub fn page_for_source(pages: &[PagePlan], offset: u32) -> usize {
    match pages.binary_search_by(|p| p.source_start.cmp(&offset)) {
        Ok(i) => i,
        Err(0) => 0,
        Err(i) => i - 1,
    }
}
