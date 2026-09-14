//! 阅读排版的**分页那一半**:测量输入模型、分页器、锚点恢复。
//!
//! 计划书 `docs/reader-layout-plan.md` §4.1。分工的那条线:
//! **字形整形与测量在 Flutter(真平台字体),装页与锚点在这里**。
//! 所以这个 crate 不依赖 Flutter、FRB、数据库和网络 —— 单测直接造行度量。
//!
//! 两个模块:
//! - [`measure`] 行度量的跨 FFI 线格式(一个自描述字节缓冲,不是上万个小对象);
//! - [`compose`] 装页、无缝分割的后置校验、锚点定位。

#![forbid(unsafe_code)]

pub mod compose;
pub mod measure;
pub mod pages_wire;

pub use compose::{
    ComposeDiagnostics, ComposeError, ComposeParams, ComposeResult, DEFAULT_MIN_SPLIT_LINES,
    MAX_BOTTOM_GAP_RATIO, PagePlan, Placement, compose, page_for_source,
};
pub use pages_wire::encode_pages;
pub use measure::{BlockMeasure, DecodeError, LineMeasure, MEASURE_SCHEMA_VERSION, MeasureBatch};

#[cfg(test)]
mod tests {
    use super::*;
    use reader_doc::ChapterDocument;

    const LINE_H: f32 = 32.0;

    /// 造一份「每 `per_line` 个 code unit 一行」的测量,行高恒定。
    /// 真实的整形在 Flutter 那边;这里只要行度量的形状对。
    fn measure(doc: &ChapterDocument, per_line: u32, upto: usize) -> MeasureBatch {
        let mut blocks = Vec::new();
        for b in doc.blocks.iter().take(upto) {
            let len = b.render_len();
            let mut lines = Vec::new();
            let mut at = 0u32;
            loop {
                let end = (at + per_line).min(len);
                lines.push(LineMeasure {
                    render_start: at,
                    render_end: end,
                    top: lines.len() as f32 * LINE_H,
                    height: LINE_H,
                });
                at = end;
                if at >= len {
                    break;
                }
            }
            blocks.push(BlockMeasure { block_id: b.id, space_before: 0.0, lines });
        }
        MeasureBatch { blocks }
    }

    /// M3b **之前**的装页口径:孤行控制关掉。M1/M2 那些逐页对照的用例继续
    /// 判它们本来在判的东西 —— 换成 M3b 的口径会让「这条用例在判什么」变模糊
    fn params(height: f32) -> ComposeParams {
        ComposeParams { viewport_height: height, scroll: false, min_split_lines: 1, bottom_align: false }
    }

    /// M3b 的产品口径:孤行控制按惯例开着
    fn zh(height: f32) -> ComposeParams {
        ComposeParams {
            viewport_height: height,
            scroll: false,
            min_split_lines: DEFAULT_MIN_SPLIT_LINES,
            bottom_align: false,
        }
    }

    /// 滚动模式:同一个视口高度,但不断页
    fn scrolling(height: f32) -> ComposeParams {
        ComposeParams { viewport_height: height, scroll: true, min_split_lines: 1, bottom_align: false }
    }

    fn body(paragraphs: usize, per_paragraph: usize) -> String {
        (0..paragraphs)
            .map(|i| format!("　　{}", "甲乙丙丁戊己庚辛".repeat(per_paragraph).chars().skip(i % 8).collect::<String>()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 计划书 §5.5 的分页不变量,一次全查
    fn assert_invariants(doc: &ChapterDocument, r: &ComposeResult) {
        assert!(!r.pages.is_empty(), "至少一页");
        assert_eq!(r.pages[0].source_start, 0, "首页页首必须是 0");
        for (i, p) in r.pages.iter().enumerate() {
            assert_eq!(p.page_index, i as u32);
            assert!(p.source_start <= p.source_end, "页范围倒挂");
        }
        for w in r.pages.windows(2) {
            assert_eq!(w[0].source_end, w[1].source_start, "页之间有空隙或重叠");
            assert!(w[1].source_start > w[0].source_start, "每页至少推进 1 个 code unit");
        }
        if r.complete {
            assert_eq!(
                r.pages.last().expect("末页").source_end,
                doc.source_utf16_len,
                "末页没盖到章末"
            );
        }
    }

    #[test]
    fn 单页装得下就只有一页() {
        let doc = ChapterDocument::build("t", "　　短短一段。").expect("doc");
        let r = compose(&doc, &measure(&doc, 100, doc.blocks.len()), params(700.0)).expect("compose");
        assert_eq!(r.pages.len(), 1);
        assert!(r.complete);
        assert!(r.diagnostics.is_clean());
        assert_invariants(&doc, &r);
    }

    #[test]
    fn 空章是一页空页() {
        let doc = ChapterDocument::build("t", "").expect("doc");
        let r = compose(&doc, &measure(&doc, 20, 1), params(700.0)).expect("compose");
        assert_eq!(r.pages.len(), 1);
        assert_eq!(r.pages[0].source_start, 0);
        assert_eq!(r.pages[0].source_end, 0);
        assert_invariants(&doc, &r);
    }

    #[test]
    fn 长章无缝分成多页() {
        let doc = ChapterDocument::build("t", &body(40, 6)).expect("doc");
        let r = compose(&doc, &measure(&doc, 20, doc.blocks.len()), params(700.0)).expect("compose");
        assert!(r.pages.len() > 3, "40 段应当排出多页,实得 {}", r.pages.len());
        assert!(r.diagnostics.is_clean());
        assert_invariants(&doc, &r);
    }

    #[test]
    fn 段落跨页续排时_origin_y_为负() {
        // 一段 10 行、页高只装得下 3 行 => 第二页从该段第 3 行起,
        // origin_y = 0 - 3*行高
        let doc = ChapterDocument::build("t", &"甲".repeat(200)).expect("doc");
        let r = compose(&doc, &measure(&doc, 20, 1), params(LINE_H * 3.0)).expect("compose");
        assert!(r.pages.len() >= 4);
        let Placement::Lines { line_start, origin_y, .. } = r.pages[1].placements[0];
        assert_eq!(line_start, 3);
        assert!((origin_y + LINE_H * 3.0).abs() < 1e-3, "origin_y = {origin_y}");
        assert_invariants(&doc, &r);
    }

    #[test]
    fn 前缀装页与全量装页逐页相同() {
        // 计划书 §6.1:首屏只量到锚点页够用的块。这条保证「先画的那几页」
        // 不会在整章补齐之后跳位置 —— 除了最后一页(它还会长)。
        let doc = ChapterDocument::build("t", &body(30, 5)).expect("doc");
        let full = compose(&doc, &measure(&doc, 20, doc.blocks.len()), params(400.0)).expect("full");
        let part = compose(&doc, &measure(&doc, 20, 12), params(400.0)).expect("part");
        assert!(!part.complete);
        assert!(part.pages.len() >= 2);
        for i in 0..part.pages.len() - 1 {
            assert_eq!(part.pages[i], full.pages[i], "第 {i} 页在补齐后变了");
        }
        assert_invariants(&doc, &part);
    }

    #[test]
    fn 单行高于整页也必须推进() {
        let doc = ChapterDocument::build("t", &"甲".repeat(100)).expect("doc");
        let batch = measure(&doc, 20, 1);
        // 页高比一行还矮
        let r = compose(&doc, &batch, params(LINE_H / 2.0)).expect("compose");
        assert_eq!(r.pages.len(), 5, "每行一页");
        assert_eq!(r.diagnostics.oversized_lines, 5);
        assert_invariants(&doc, &r);
    }

    /// 块前间距**在页顶折叠**。少了这一条,每页顶上都空一个段距 ——
    /// 既不是排版意图,还让每页少装一行。
    #[test]
    fn 块前间距在页顶折叠() {
        // 4 段各 1 行(行高 32),页高恰好 2 行 + 1 个段距(16)
        let doc = ChapterDocument::build("t", "甲\n乙\n丙\n丁").expect("doc");
        let mut batch = measure(&doc, 20, doc.blocks.len());
        for b in batch.blocks.iter_mut() {
            b.space_before = 16.0;
        }
        let r = compose(&doc, &batch, params(LINE_H * 2.0 + 16.0)).expect("compose");
        // 每页 2 段:页顶那段不加间距,第二段加 16 => 32+16+32 = 80 = 页高
        assert_eq!(r.pages.len(), 2, "实得 {} 页", r.pages.len());
        assert_eq!(r.pages[0].placements.len(), 2);
        assert_eq!(r.pages[1].placements.len(), 2);
        assert!(r.diagnostics.is_clean());
        assert_invariants(&doc, &r);
    }

    /// 页顶折叠不是「第一页才折叠」:每一页的页顶都折叠
    #[test]
    fn 每页页顶的第一块都不带间距() {
        let doc = ChapterDocument::build("t", &body(12, 3)).expect("doc");
        let mut batch = measure(&doc, 10, doc.blocks.len());
        for b in batch.blocks.iter_mut() {
            b.space_before = 40.0;
        }
        let r = compose(&doc, &batch, params(300.0)).expect("compose");
        assert!(r.pages.len() > 2);
        for p in &r.pages {
            let Placement::Lines { origin_y, line_start, .. } = p.placements[0];
            // 页首那一段的第一行必须落在 y = 0(块内续排时是 origin_y + top)
            let top = line_start as f32 * LINE_H;
            assert!((origin_y + top).abs() < 1e-3, "第 {} 页顶有空白", p.page_index);
        }
        assert_invariants(&doc, &r);
    }

    /// 标题块的间距比段距大,照样只是「一个数」—— 装页器不认识标题
    #[test]
    fn 标题块靠自己的块前间距拉开() {
        let doc = ChapterDocument::build_with_title("t", "第一章", "甲\n乙").expect("doc");
        assert_eq!(doc.blocks.len(), 3);
        let mut batch = measure(&doc, 20, doc.blocks.len());
        batch.blocks[1].space_before = 50.0; // 标题下间距
        batch.blocks[2].space_before = 8.0; // 段距
        let r = compose(&doc, &batch, params(700.0)).expect("compose");
        assert_eq!(r.pages.len(), 1);
        let ys: Vec<f32> = r
            .pages[0]
            .placements
            .iter()
            .map(|p| {
                let Placement::Lines { origin_y, .. } = p;
                *origin_y
            })
            .collect();
        assert!((ys[0] - 0.0).abs() < 1e-3);
        assert!((ys[1] - (LINE_H + 50.0)).abs() < 1e-3, "标题下间距没落上:{ys:?}");
        assert!((ys[2] - (LINE_H * 2.0 + 50.0 + 8.0)).abs() < 1e-3, "段距没落上:{ys:?}");
        assert_invariants(&doc, &r);
    }

    // ---------------- 滚动模式(M2c) ----------------

    /// 整章排成**一页**,`origin_y` 是章内绝对坐标 —— 不是「每页从 0 起」
    #[test]
    fn 滚动模式整章一页且_origin_y_是绝对坐标() {
        let doc = ChapterDocument::build("t", &body(30, 5)).expect("doc");
        let batch = measure(&doc, 20, doc.blocks.len());
        let paged = compose(&doc, &batch, params(400.0)).expect("paged");
        let sc = compose(&doc, &batch, scrolling(400.0)).expect("scroll");

        assert!(paged.pages.len() > 3, "分页那条本来就是多页");
        assert_eq!(sc.pages.len(), 1, "滚动只有一页");
        assert_eq!(sc.pages[0].source_start, 0);
        assert_eq!(sc.pages[0].source_end, doc.source_utf16_len);
        assert_eq!(sc.pages[0].placements.len(), doc.blocks.len(), "每块一段,没有断行");
        assert!(sc.diagnostics.is_clean());

        // origin_y 单调不减,而且一直长到整章的高度(分页那条是每页回到 0)
        let mut prev = f32::NEG_INFINITY;
        for pl in &sc.pages[0].placements {
            let Placement::Lines { origin_y, .. } = pl;
            assert!(*origin_y >= prev, "origin_y 回退了:{origin_y} < {prev}");
            prev = *origin_y;
        }
        assert!(prev > 400.0, "整章高度该远超一屏,实得 {prev}");
        assert_invariants(&doc, &sc);
    }

    /// **同一份行度量,两种模式**:滚动那条的摆放就是把分页那条各页的
    /// `origin_y` 加上「这一页之前的总高」。这条钉的是「两种模式共用一套测量」
    /// (计划书 M2c:滚动与分页共用 ChapterDocument)
    #[test]
    fn 两种模式共用同一份行度量() {
        let doc = ChapterDocument::build("t", "甲\n乙\n丙\n丁\n戊").expect("doc");
        let mut batch = measure(&doc, 20, doc.blocks.len());
        for b in batch.blocks.iter_mut() {
            b.space_before = 10.0;
        }
        let sc = compose(&doc, &batch, scrolling(LINE_H * 2.0)).expect("scroll");
        // 5 块各 1 行:块间 10 的间距在滚动下**一个都不折叠**(没有页顶)
        let ys: Vec<f32> = sc.pages[0]
            .placements
            .iter()
            .map(|p| {
                let Placement::Lines { origin_y, .. } = p;
                *origin_y
            })
            .collect();
        for (i, y) in ys.iter().enumerate() {
            let want = i as f32 * (LINE_H + 10.0);
            assert!((y - want).abs() < 1e-3, "第 {i} 块该在 {want},实得 {y}");
        }
    }

    /// 前缀装页在滚动下也成立:那一页往下长,**前面的摆放一位不动**。
    /// 首屏据此在整章测完之前就能画(§6.1),分页与滚动同一条路
    #[test]
    fn 滚动的前缀装页只往下长() {
        let doc = ChapterDocument::build("t", &body(30, 5)).expect("doc");
        let full = compose(&doc, &measure(&doc, 20, doc.blocks.len()), scrolling(400.0))
            .expect("full");
        let part = compose(&doc, &measure(&doc, 20, 12), scrolling(400.0)).expect("part");
        assert!(!part.complete);
        assert_eq!(part.pages.len(), 1);
        let (a, b) = (&part.pages[0].placements, &full.pages[0].placements);
        assert_eq!(a.len(), 12);
        assert_eq!(&b[..a.len()], &a[..], "补齐之后前面的摆放变了");
        assert_eq!(part.pages[0].source_start, 0);
        assert_invariants(&doc, &part);
    }

    /// 滚动下**不报诊断**:没有页,也就没有「单行高于整页」和「并页」。
    /// 分页那条会为一行比页还高的情形记一笔,滚动那条不该跟着记
    #[test]
    fn 滚动下没有超高行也没有并页() {
        let doc = ChapterDocument::build("t", &"甲".repeat(100)).expect("doc");
        let batch = measure(&doc, 20, 1);
        // 页高比一行还矮 —— 分页那条会记 5 次超高
        assert_eq!(compose(&doc, &batch, params(LINE_H / 2.0)).expect("p").diagnostics.oversized_lines, 5);
        let sc = compose(&doc, &batch, scrolling(LINE_H / 2.0)).expect("scroll");
        assert!(sc.diagnostics.is_clean());
        assert_eq!(sc.pages.len(), 1);
    }

    #[test]
    fn 滚动下的空章仍是一页空页() {
        let doc = ChapterDocument::build("t", "").expect("doc");
        let r = compose(&doc, &measure(&doc, 20, 1), scrolling(700.0)).expect("scroll");
        assert_eq!(r.pages.len(), 1);
        assert_eq!(r.pages[0].source_start, 0);
        assert_eq!(r.pages[0].source_end, 0);
        assert_invariants(&doc, &r);
    }

    /// 滚动模式下视口高度**照样要合法**:它不参与装页,但它是同一份排版条件的
    /// 一部分,不该在这条路上变成「随便传什么都行」
    #[test]
    fn 滚动模式也拒绝退化的视口() {
        let doc = ChapterDocument::build("t", "甲").expect("doc");
        let b = measure(&doc, 20, 1);
        assert_eq!(compose(&doc, &b, scrolling(0.0)).unwrap_err(), ComposeError::BadViewport);
        assert_eq!(compose(&doc, &b, scrolling(f32::NAN)).unwrap_err(), ComposeError::BadViewport);
        assert_eq!(
            compose(&doc, &b, scrolling(f32::INFINITY)).unwrap_err(),
            ComposeError::BadViewport
        );
    }

    // ---------------- 孤行控制(M3b) ----------------

    /// 一页里各块各占几行 —— 孤行控制的用例只关心这个形状
    fn shape(r: &ComposeResult) -> Vec<Vec<(u64, u32, u32)>> {
        r.pages
            .iter()
            .map(|p| {
                p.placements
                    .iter()
                    .map(|pl| {
                        let Placement::Lines { block_id, line_start, line_end, .. } = pl;
                        (*block_id, *line_start, *line_end)
                    })
                    .collect()
            })
            .collect()
    }

    /// **两行的段落永远不拆**:2 行拆出来两边各 1 行,两边都不够 `min = 2`。
    /// 关掉这条规则(`params`)时它会被拆开 —— 那正是 M3b 之前的样子
    #[test]
    fn 两行的段落不拆() {
        // 块 0 一行,块 1 两行;页高恰好两行
        let doc = ChapterDocument::build("t", "甲\n乙丙丁").expect("doc");
        let batch = measure(&doc, 2, doc.blocks.len());
        assert_eq!(batch.blocks[1].lines.len(), 2);

        let off = compose(&doc, &batch, params(LINE_H * 2.0)).expect("off");
        assert_eq!(shape(&off), vec![vec![(0, 0, 1), (1, 0, 1)], vec![(1, 1, 2)]], "旧口径:拆开");

        let on = compose(&doc, &batch, zh(LINE_H * 2.0)).expect("on");
        assert_eq!(shape(&on), vec![vec![(0, 0, 1)], vec![(1, 0, 2)]], "整段下推");
        assert!(on.diagnostics.is_clean());
        assert_invariants(&doc, &on);
    }

    /// **页尾孤行**:四行的段落在只装得下三行的页上,切点往回拉成 2 + 2
    #[test]
    fn 段尾只剩一行时把切点往回拉() {
        let doc = ChapterDocument::build("t", &"甲".repeat(8)).expect("doc");
        let batch = measure(&doc, 2, 1);
        assert_eq!(batch.blocks[0].lines.len(), 4);

        let off = compose(&doc, &batch, params(LINE_H * 3.0)).expect("off");
        assert_eq!(shape(&off), vec![vec![(0, 0, 3)], vec![(0, 3, 4)]], "旧口径:3 + 1");

        let on = compose(&doc, &batch, zh(LINE_H * 3.0)).expect("on");
        assert_eq!(shape(&on), vec![vec![(0, 0, 2)], vec![(0, 2, 4)]], "2 + 2");
        assert_invariants(&doc, &on);
    }

    /// **页首孤行**:段落刚在页上开了个头就只留得下一行 —— 整段下推
    #[test]
    fn 段首只留得下一行时整段下推() {
        // 块 0 两行、块 1 四行;页高三行
        let doc = ChapterDocument::build("t", "甲乙丙丁\n戊己庚辛壬癸子丑").expect("doc");
        let batch = measure(&doc, 2, doc.blocks.len());
        assert_eq!((batch.blocks[0].lines.len(), batch.blocks[1].lines.len()), (2, 4));

        let off = compose(&doc, &batch, params(LINE_H * 3.0)).expect("off");
        assert_eq!(shape(&off)[0], vec![(0, 0, 2), (1, 0, 1)], "旧口径:段首留一行在页底");

        let on = compose(&doc, &batch, zh(LINE_H * 3.0)).expect("on");
        assert_eq!(
            shape(&on),
            vec![vec![(0, 0, 2)], vec![(1, 0, 2)], vec![(1, 2, 4)]],
            "整段下推,然后按 2 + 2 拆"
        );
        assert_invariants(&doc, &on);
    }

    /// **页还空着时下推没有去处**:照原样切,而且不会死循环 ——
    /// 「整块下推」若不看这一条,块比页高时会把自己推回同一个空页,推到天荒地老
    #[test]
    fn 页还空着时下推没有去处() {
        let doc = ChapterDocument::build("t", &"甲".repeat(10)).expect("doc");
        let batch = measure(&doc, 2, 1); // 5 行
        // 页高只有一行:`min = 2` 一次都满足不了
        let r = compose(&doc, &batch, zh(LINE_H)).expect("compose");
        assert_eq!(shape(&r).len(), 5, "每行一页");
        assert!(r.diagnostics.is_clean(), "做不到不是异常:{:?}", r.diagnostics);
        assert_invariants(&doc, &r);
    }

    /// 续排来的那几行**不是段首**,所以页首那一条不适用 —— 只要能推进一行就行。
    /// 少了这条,长段落在窄页上会一路把自己往后推
    #[test]
    fn 续排块不受页首孤行约束() {
        // 一段 7 行,页高 3 行
        let doc = ChapterDocument::build("t", &"甲".repeat(14)).expect("doc");
        let batch = measure(&doc, 2, 1);
        assert_eq!(batch.blocks[0].lines.len(), 7);
        let r = compose(&doc, &batch, zh(LINE_H * 3.0)).expect("compose");
        // 0..3 | 3..5(尾巴要留 2 行) | 5..7
        assert_eq!(shape(&r), vec![vec![(0, 0, 3)], vec![(0, 3, 5)], vec![(0, 5, 7)]]);
        assert_invariants(&doc, &r);
    }

    /// 孤行控制**不破前缀装页**:它只看本块自己的行数,不看后面还有什么
    #[test]
    fn 孤行控制下前缀装页仍与全量逐页相同() {
        let doc = ChapterDocument::build("t", &body(30, 5)).expect("doc");
        let full = compose(&doc, &measure(&doc, 7, doc.blocks.len()), zh(400.0)).expect("full");
        let part = compose(&doc, &measure(&doc, 7, 12), zh(400.0)).expect("part");
        assert!(!part.complete);
        assert!(part.pages.len() >= 2);
        for i in 0..part.pages.len() - 1 {
            assert_eq!(part.pages[i], full.pages[i], "第 {i} 页在补齐后变了");
        }
        assert_invariants(&doc, &part);
    }

    /// 滚动模式下孤行控制是**空操作**:页高无穷,一个块都不拆
    #[test]
    fn 滚动下孤行控制不改变任何东西() {
        let doc = ChapterDocument::build("t", &body(20, 4)).expect("doc");
        let batch = measure(&doc, 9, doc.blocks.len());
        let a = compose(&doc, &batch, scrolling(400.0)).expect("a");
        let b = compose(
            &doc,
            &batch,
            ComposeParams {
                viewport_height: 400.0,
                scroll: true,
                min_split_lines: DEFAULT_MIN_SPLIT_LINES,
                bottom_align: true,
            },
        )
        .expect("b");
        assert_eq!(a.pages, b.pages);
    }

    /// 图片块只有一条(空)行,拆不动 —— 孤行控制不该把它挪走
    #[test]
    fn 孤行控制不动图片块() {
        let doc = ChapterDocument::build("t", "甲乙丙\n<img src=\"a\">\n丁戊己").expect("doc");
        let mut batch = measure(&doc, 20, 3);
        as_image(&mut batch, 1, 200.0);
        let r = compose(&doc, &batch, zh(700.0)).expect("compose");
        assert_eq!(r.pages.len(), 1);
        assert_eq!(shape(&r)[0], vec![(0, 0, 1), (1, 0, 1), (2, 0, 1)]);
        assert_invariants(&doc, &r);
    }

    /// 随机输入下,**孤行控制这一档**的不变量同样恒成立,而且规则真的兑现了:
    /// 任何被拆开的块,两边各至少 `min` 行(除了「做不到」的那些页)
    #[test]
    fn 孤行控制下随机输入的不变量恒成立() {
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as u32
        };
        let min = DEFAULT_MIN_SPLIT_LINES;
        for _ in 0..300 {
            let paragraphs = 1 + next() as usize % 20;
            let text = (0..paragraphs)
                .map(|_| "甲".repeat(next() as usize % 60))
                .collect::<Vec<_>>()
                .join("\n");
            let doc = ChapterDocument::build("t", &text).expect("doc");
            let per_line = 1 + next() % 25;
            let vh = 20.0 + (next() % 400) as f32;
            let batch = measure(&doc, per_line, doc.blocks.len());
            let r = compose(
                &doc,
                &batch,
                ComposeParams {
                    viewport_height: vh,
                    scroll: false,
                    min_split_lines: min,
                    bottom_align: false,
                },
            )
            .expect("compose");
            assert_invariants(&doc, &r);

            // 规则本身:每个块被切成了哪几段
            let mut runs: Vec<Vec<(u32, u32, usize)>> = vec![Vec::new(); doc.blocks.len()];
            for (pi, p) in r.pages.iter().enumerate() {
                for pl in &p.placements {
                    let Placement::Lines { block_id, line_start, line_end, .. } = pl;
                    runs[*block_id as usize].push((*line_start, *line_end, pi));
                }
            }
            for (bi, segs) in runs.iter().enumerate() {
                let len = batch.blocks[bi].lines.len();
                if segs.len() < 2 {
                    continue; // 没被拆
                }
                // 两档「做不到」要先排掉,它们不是规则没兑现:
                //   - 一页装不下 `min` 行 —— 下推没有去处;
                //   - 整块的行数不到 `2 * min` —— 拆成两半必然有一半不够。
                //     3 行的段落落在只装 2 行的页上就是这一种:2+1 与 1+2 都违规
                let fits = (vh / LINE_H).floor() as usize;
                if fits < min as usize || len < 2 * min as usize {
                    continue;
                }
                for (k, (s, e, _)) in segs.iter().enumerate() {
                    let head = k == 0;
                    let tail = k == segs.len() - 1;
                    let n = (e - s) as usize;
                    if head {
                        assert!(n >= min as usize, "块 {bi}(共 {len} 行)段首只留了 {n} 行");
                    }
                    if tail {
                        assert!(n >= min as usize, "块 {bi}(共 {len} 行)段尾只剩了 {n} 行");
                    }
                }
            }
        }
    }

    // ---------------- 页底对齐(M3d) ----------------

    /// 页底对齐口径:孤行控制照旧开着(它正是页底空白的来源之一)
    fn flush(height: f32) -> ComposeParams {
        ComposeParams {
            viewport_height: height,
            scroll: false,
            min_split_lines: DEFAULT_MIN_SPLIT_LINES,
            bottom_align: true,
        }
    }

    /// 本页每一行的行底(页内 y),按摆放序
    fn line_bottoms(r: &ComposeResult, batch: &MeasureBatch, page: usize) -> Vec<f32> {
        let mut out = Vec::new();
        for pl in &r.pages[page].placements {
            let Placement::Lines { block_id, line_start, line_end, origin_y, line_gap, .. } = pl;
            let lines = &batch.blocks[*block_id as usize].lines;
            for k in *line_start..*line_end {
                let l = &lines[k as usize];
                out.push(origin_y + l.top + (k - line_start) as f32 * line_gap + l.height);
            }
        }
        out
    }

    /// 「差一点点就满」的页被摊平:末行行底 == 页高,而且行序不乱
    #[test]
    fn 页底对齐把差一点点的那截摊进行距() {
        // 一段 20 行(行高 32);页高 500 => 装 15 行,剩 20
        let doc = ChapterDocument::build("t", &"甲".repeat(40)).expect("doc");
        let batch = measure(&doc, 2, 1);
        assert_eq!(batch.blocks[0].lines.len(), 20);

        let ragged = compose(&doc, &batch, zh(500.0)).expect("ragged");
        let bs = line_bottoms(&ragged, &batch, 0);
        assert_eq!(bs.len(), 15);
        assert!((bs[14] - 480.0).abs() < 1e-3, "不对齐时页底空 20:{}", bs[14]);

        let flushed = compose(&doc, &batch, flush(500.0)).expect("flush");
        let fs = line_bottoms(&flushed, &batch, 0);
        assert_eq!(fs.len(), 15, "对齐不该改变装了几行");
        assert!((fs[14] - 500.0).abs() < 1e-3, "末行没贴到页底:{}", fs[14]);
        for w in fs.windows(2) {
            assert!(w[1] > w[0]);
        }
        let step = fs[1] - fs[0];
        for w in fs.windows(2) {
            assert!((w[1] - w[0] - step).abs() < 1e-3, "行距不均匀:{fs:?}");
        }
        // **页范围一位没动**:对齐只挪 y,不改这一页装了什么
        assert_eq!(
            ragged.pages.iter().map(|p| (p.source_start, p.source_end)).collect::<Vec<_>>(),
            flushed.pages.iter().map(|p| (p.source_start, p.source_end)).collect::<Vec<_>>(),
        );
        assert_invariants(&doc, &flushed);
    }

    /// **末页不对齐**:它本来就短,分批测量时还会往下长
    #[test]
    fn 末页不做页底对齐() {
        let doc = ChapterDocument::build("t", &"甲".repeat(40)).expect("doc");
        let batch = measure(&doc, 2, 1);
        let r = compose(&doc, &batch, flush(500.0)).expect("compose");
        assert!(r.pages.len() >= 2);
        let last = r.pages.len() - 1;
        for pl in &r.pages[last].placements {
            let Placement::Lines { line_gap, .. } = pl;
            assert_eq!(*line_gap, 0.0, "末页被摊了");
        }
    }

    /// **本来就短的页不摊**:孤行控制把一块整块推下去时,剩的空白比一行还多,
    /// 撑开行距只会把「短」变成「行距怪」
    #[test]
    fn 剩得比一行还多的页不摊() {
        // 块 0 两行、块 1 四行;页高三行 => 孤行控制把块 1 整块推下去,
        // 第 0 页只剩两行、空着一整行
        let doc = ChapterDocument::build("t", "甲乙丙丁\n戊己庚辛壬癸子丑").expect("doc");
        let batch = measure(&doc, 2, doc.blocks.len());
        let r = compose(&doc, &batch, flush(LINE_H * 3.0)).expect("compose");
        assert_eq!(r.pages[0].placements.len(), 1);
        let Placement::Lines { line_gap, .. } = r.pages[0].placements[0];
        assert_eq!(line_gap, 0.0, "两行的页要摊一整行,每道行距撑 100% —— 不该摊");
    }

    /// 一行的页没有「行与行之间」可摊 —— 不摊,也不除以零
    #[test]
    fn 只有一行的页不摊() {
        let doc =
            ChapterDocument::build("t", "<img src=\"a\">\n<img src=\"b\">\n甲").expect("doc");
        let mut batch = measure(&doc, 20, 3);
        as_image(&mut batch, 0, 100.0);
        as_image(&mut batch, 1, 100.0);
        let r = compose(&doc, &batch, flush(120.0)).expect("compose");
        for p in &r.pages {
            for pl in &p.placements {
                let Placement::Lines { line_gap, .. } = pl;
                assert!(line_gap.is_finite(), "line_gap 不是有限值");
            }
        }
        let Placement::Lines { line_gap, .. } = r.pages[0].placements[0];
        assert_eq!(line_gap, 0.0);
        assert_invariants(&doc, &r);
    }

    /// 页底对齐**只挪 y,不改分页**:同一份输入,开与关的页范围逐页相同
    #[test]
    fn 页底对齐不改变分页() {
        let doc = ChapterDocument::build("t", &body(30, 5)).expect("doc");
        let batch = measure(&doc, 7, doc.blocks.len());
        for vh in [200.0, 333.0, 500.0, 777.0] {
            let a = compose(&doc, &batch, zh(vh)).expect("a");
            let b = compose(&doc, &batch, flush(vh)).expect("b");
            assert_eq!(a.pages.len(), b.pages.len(), "视口 {vh}");
            for (x, y) in a.pages.iter().zip(b.pages.iter()) {
                assert_eq!((x.source_start, x.source_end), (y.source_start, y.source_end));
                assert_eq!(x.placements.len(), y.placements.len());
            }
            assert_invariants(&doc, &b);
        }
    }

    /// 滚动模式没有「页底」这回事
    #[test]
    fn 滚动下不做页底对齐() {
        let doc = ChapterDocument::build("t", &body(20, 4)).expect("doc");
        let batch = measure(&doc, 9, doc.blocks.len());
        let r = compose(
            &doc,
            &batch,
            ComposeParams {
                viewport_height: 400.0,
                scroll: true,
                min_split_lines: DEFAULT_MIN_SPLIT_LINES,
                bottom_align: true,
            },
        )
        .expect("scroll");
        for pl in &r.pages[0].placements {
            let Placement::Lines { line_gap, .. } = pl;
            assert_eq!(*line_gap, 0.0);
        }
    }

    /// 随机输入:摊过的页末行**恰好**贴底,而且摊的一定不是末页
    #[test]
    fn 页底对齐下随机输入的不变量恒成立() {
        let mut seed: u64 = 0x1234_5678_9ABC_DEF0;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as u32
        };
        for _ in 0..300 {
            let paragraphs = 1 + next() as usize % 20;
            let text = (0..paragraphs)
                .map(|_| "甲".repeat(next() as usize % 60))
                .collect::<Vec<_>>()
                .join("\n");
            let doc = ChapterDocument::build("t", &text).expect("doc");
            let per_line = 1 + next() % 25;
            let vh = 20.0 + (next() % 400) as f32;
            let batch = measure(&doc, per_line, doc.blocks.len());
            let r = compose(&doc, &batch, flush(vh)).expect("compose");
            assert_invariants(&doc, &r);
            for i in 0..r.pages.len() {
                let bs = line_bottoms(&r, &batch, i);
                if bs.is_empty() {
                    continue;
                }
                let touched = r.pages[i].placements.iter().any(|pl| {
                    let Placement::Lines { line_gap, .. } = pl;
                    *line_gap > 0.0
                });
                if !touched {
                    continue;
                }
                assert!(i + 1 < r.pages.len(), "末页被摊了");
                let last = *bs.last().expect("非空");
                assert!((last - vh).abs() <= 1e-2, "第 {i} 页摊完没贴底:{last} vs {vh}");
                for w in bs.windows(2) {
                    assert!(w[1] > w[0], "第 {i} 页行序乱了");
                }
            }
        }
    }

    // ---------------- 块级图片(M2b) ----------------

    /// 把某一块换成「一张 `h` 高的图」:**一条空行**。
    /// 装页器不认识图片,所以这里没有任何图片专用的入口
    fn as_image(batch: &mut MeasureBatch, block: usize, h: f32) {
        batch.blocks[block].lines =
            vec![LineMeasure { render_start: 0, render_end: 0, top: 0.0, height: h }];
    }

    /// 图片就是「一条空行的块」——装页器一位都不用改
    #[test]
    fn 图片块按一条空行装页() {
        let doc = ChapterDocument::build("t", "甲乙丙\n<img src=\"a\">\n丁戊己").expect("doc");
        assert_eq!(doc.blocks.len(), 3);
        let mut batch = measure(&doc, 20, 3);
        as_image(&mut batch, 1, 200.0);
        let r = compose(&doc, &batch, params(700.0)).expect("compose");
        assert_eq!(r.pages.len(), 1);
        assert!(r.diagnostics.is_clean());
        let Placement::Lines { block_id, line_start, line_end, origin_y, .. } =
            r.pages[0].placements[1];
        assert_eq!((block_id, line_start, line_end), (1, 0, 1));
        assert!((origin_y - LINE_H).abs() < 1e-3, "图片该接在第一段下面:{origin_y}");
        // 第三块从图片下方 200 起
        let Placement::Lines { origin_y: y2, .. } = r.pages[0].placements[2];
        assert!((y2 - (LINE_H + 200.0)).abs() < 1e-3, "{y2}");
        assert_invariants(&doc, &r);
    }

    /// 一整页高的图片自己占一页,而且**不报 oversized 诊断** ——
    /// 超高图片是 Dart 那侧压到视口以内之后才送进来的(计划书 M2b「超高图片策略」),
    /// 诊断位留给真正的异常
    #[test]
    fn 满页高的图片自己占一页() {
        let doc = ChapterDocument::build("t", "甲乙丙\n<img src=\"a\">\n丁戊己").expect("doc");
        let mut batch = measure(&doc, 20, 3);
        as_image(&mut batch, 1, 300.0);
        let r = compose(&doc, &batch, params(300.0)).expect("compose");
        assert_eq!(r.pages.len(), 3, "文字、图片、文字各一页");
        assert!(r.diagnostics.is_clean(), "{:?}", r.diagnostics);
        assert_eq!(r.pages[1].placements.len(), 1);
        let Placement::Lines { block_id, origin_y, .. } = r.pages[1].placements[0];
        assert_eq!(block_id, 1);
        assert!(origin_y.abs() < 1e-3, "图片页的图片该贴着页顶:{origin_y}");
        assert_invariants(&doc, &r);
    }

    /// 图片块的 source 区间非空(它盖着整个 `<img …>` 标签),所以
    /// 「每页至少推进 1 个 code unit」不用为它开口子 —— 这正是标题**不做**
    /// 虚拟块的那条理由的另一面(虚拟块的区间是空的,当场就被并页)
    #[test]
    fn 连着的图片每张一页也不会被并掉() {
        let doc = ChapterDocument::build("t", "<img src=\"a\"><img src=\"b\"><img src=\"c\">")
            .expect("doc");
        assert_eq!(doc.blocks.len(), 3);
        let mut batch = measure(&doc, 20, 3);
        for i in 0..3 {
            as_image(&mut batch, i, 400.0);
        }
        let r = compose(&doc, &batch, params(400.0)).expect("compose");
        assert_eq!(r.pages.len(), 3, "三张图三页");
        assert_eq!(r.diagnostics.merged_pages, 0);
        assert_invariants(&doc, &r);
    }

    #[test]
    fn 只有一张图的章也是一页() {
        let doc = ChapterDocument::build("t", "<img src=\"a\">").expect("doc");
        let mut batch = measure(&doc, 20, 1);
        as_image(&mut batch, 0, 120.0);
        let r = compose(&doc, &batch, params(700.0)).expect("compose");
        assert_eq!(r.pages.len(), 1);
        assert_eq!(r.pages[0].source_start, 0);
        assert_eq!(r.pages[0].source_end, doc.source_utf16_len);
        assert_invariants(&doc, &r);
    }

    /// 图片块的行区间只有 `0..0` 合法:Dart 报了别的当场判死
    #[test]
    fn 图片块的行区间越界当场判死() {
        let doc = ChapterDocument::build("t", "<img src=\"a\">").expect("doc");
        let mut batch = measure(&doc, 20, 1);
        as_image(&mut batch, 0, 120.0);
        batch.blocks[0].lines[0].render_end = 1;
        assert_eq!(
            compose(&doc, &batch, params(700.0)).unwrap_err(),
            ComposeError::LineOutOfRange { block: 0, line: 0 }
        );
    }

    #[test]
    fn 块前间距非法时拒绝布局() {
        let doc = ChapterDocument::build("t", "甲\n乙").expect("doc");
        for bad in [-1.0, f32::NAN, f32::INFINITY] {
            let mut batch = measure(&doc, 20, 2);
            batch.blocks[1].space_before = bad;
            assert_eq!(
                compose(&doc, &batch, params(700.0)).unwrap_err(),
                ComposeError::BadSpacing { block: 1 },
                "{bad} 应当被拒"
            );
        }
    }

    #[test]
    fn 视口退化时拒绝布局() {
        let doc = ChapterDocument::build("t", "甲").expect("doc");
        let b = measure(&doc, 20, 1);
        assert_eq!(compose(&doc, &b, params(0.0)).unwrap_err(), ComposeError::BadViewport);
        assert_eq!(compose(&doc, &b, params(-1.0)).unwrap_err(), ComposeError::BadViewport);
        assert_eq!(compose(&doc, &b, params(f32::NAN)).unwrap_err(), ComposeError::BadViewport);
    }

    #[test]
    fn 块序对不上当场判死() {
        let doc = ChapterDocument::build("t", "甲\n乙").expect("doc");
        let mut b = measure(&doc, 20, 2);
        b.blocks[1].block_id = 77;
        assert_eq!(compose(&doc, &b, params(700.0)).unwrap_err(), ComposeError::BlockMismatch { at: 1 });
    }

    #[test]
    fn 行区间越界当场判死() {
        let doc = ChapterDocument::build("t", "甲乙").expect("doc");
        let mut b = measure(&doc, 20, 1);
        b.blocks[0].lines[0].render_end = 99;
        assert_eq!(
            compose(&doc, &b, params(700.0)).unwrap_err(),
            ComposeError::LineOutOfRange { block: 0, line: 0 }
        );
    }

    #[test]
    fn 锚点定位到含它的那一页() {
        let doc = ChapterDocument::build("t", &body(30, 5)).expect("doc");
        let r = compose(&doc, &measure(&doc, 20, doc.blocks.len()), params(700.0)).expect("compose");
        for p in &r.pages {
            assert_eq!(page_for_source(&r.pages, p.source_start), p.page_index as usize);
            if p.source_end > p.source_start {
                assert_eq!(page_for_source(&r.pages, p.source_end - 1), p.page_index as usize);
            }
        }
        // 章末锚点落在末页
        assert_eq!(
            page_for_source(&r.pages, doc.source_utf16_len),
            r.pages.len() - 1
        );
    }

    #[test]
    fn 同一份输入装出同一份页() {
        let doc = ChapterDocument::build("t", &body(25, 4)).expect("doc");
        let b = measure(&doc, 17, doc.blocks.len());
        let a = compose(&doc, &b, params(613.0)).expect("a");
        let c = compose(&doc, &b, params(613.0)).expect("c");
        assert_eq!(a.pages, c.pages);
    }

    /// 随机块序 + 随机行度量:不变量必须都成立
    #[test]
    fn 随机输入下不变量恒成立() {
        let mut seed: u64 = 0x243F_6A88_85A3_08D3;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as u32
        };
        for _ in 0..300 {
            let paragraphs = 1 + next() as usize % 20;
            let text = (0..paragraphs)
                .map(|_| "甲".repeat(next() as usize % 60))
                .collect::<Vec<_>>()
                .join("\n");
            let doc = ChapterDocument::build("t", &text).expect("doc");
            let per_line = 1 + next() % 25;
            let upto = 1 + next() as usize % doc.blocks.len();
            let vh = 20.0 + (next() % 400) as f32;
            let batch = measure(&doc, per_line, upto);
            let mut batch = batch;
            for b in batch.blocks.iter_mut() {
                b.space_before = (next() % 12) as f32;
            }
            let r = compose(&doc, &batch, ComposeParams { viewport_height: vh, scroll: false, min_split_lines: 1, bottom_align: false })
                .expect("compose");
            assert_invariants(&doc, &r);
            // 同一份输入换成滚动:不变量同样成立,而且只有一页
            let sc = compose(&doc, &batch, ComposeParams { viewport_height: vh, scroll: true, min_split_lines: 1, bottom_align: false })
                .expect("scroll");
            assert_eq!(sc.pages.len(), 1);
            assert!(sc.diagnostics.is_clean(), "滚动下不该有诊断:{:?}", sc.diagnostics);
            assert_invariants(&doc, &sc);
        }
    }
}
