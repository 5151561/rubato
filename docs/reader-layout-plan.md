# Rubato 阅读正文排版引擎计划

> 状态：**M0～M4 全部收盘**，剩下的只有 §8 的 M5 那道门。
> **2026-09-02 又清了一轮账**（§8.6）：四十来条逐条对着代码核过，还了六笔、
> 核出三条记错了的、把十一条重新标成「是决策不是债」。
> **macOS 与参考机两遍都过了**（同日补跑），没有空档。
> M2 分 M2a/M2b/M2c 三片；M3 分 M3a～M3d 四片 + M3e 的 spike；
> M4 分五片：M4a 命中测试 / M4b 选择与复制 / M4c 高亮、下划线与落库 /
> M4d 搜索命中 / M4e 翻页动画。
> **标点悬挂与压缩按 spike 的实测不做**，门并到 M5——理由见 M3e
>
> 日期：2026-08-31；2026-09-01 按实现侧核查修正协议与判据，同日落地 M0、M1 与 M2a；
> 2026-09-02 M4 收盘（M4a～M4e），同日清账一轮（§8.6）
>
> 算法与语料参考：`LegadoTeam/legado@3046111c`（仓库 `judge/engine` 冻结快照）——
> **只作参考，不作裁判**，理由见 §3

## 1. 决策

Rubato 的阅读排版采用以下边界：

- **阅读位置与正文文本契约对齐 Legado；排版规则对齐中文排版惯例，架构与交互自行设计**；
- **Rust 持有正文文档模型、分页策略、阅读锚点、缓存与预排版状态**；
- **Flutter 使用真实平台字体完成字形整形与测量，并负责 Canvas 绘制、命中测试、选择和动画**；
- 一次重排允许两次批量 FFI 往返；进入 Rust 断行（§7.2 第二阶段）后放宽到三次，
  **翻页热路径始终不得跨 FFI**；
  （M1 落地口径：**重排**确实是两次装页；**换章**是三次 —— 多一次
  `prepareChapter`，那是另一件事，见 M1 的出入第 6 条）；
- 首版不做 Rust glyph 渲染，不把每个字符或 glyph 的绘制指令作为 DTO 传给 Flutter；
- 只有混合方案经基准证明仍不达标，才启动“Rust 排字 + 原生纹理渲染”技术验证。

这不是把现有 Dart 分页循环翻译成 Rust。当前
`app/lib/pages/reader_page.dart::_paginate` 每生成一页都会重新 layout 剩余全文，
长章节重复计算严重。第一目标是把排版改成按文档块一次测量、线性分页、结果缓存。

## 2. 目标与非目标

### 2.1 目标

1. 长章节排版复杂度随正文长度近似线性增长，不再随页数重复布局剩余全文。
2. 字号、字体、行距、窗口和横竖屏变化时，以稳定正文锚点恢复阅读位置。
3. Android、macOS、Windows 共用 Rust 文档与分页策略，同时使用各平台真实字体渲染。
4. 当前页、前一页、后一页可直接绘制；普通翻页在提交下一帧前不创建新 Paragraph、
   不切全文子串、不等待 Rust；进度在帧外合并写入。
5. 绘制协议从第一版就是**行级**的（§5.4）：后续加入中文禁则、两端对齐、标点悬挂/压缩、
   图片、划线与选择时只换「行从哪来」，不换 `PagePlan` 的形状。
6. 中文排版规则写成独立、可单测的纯函数 Rust 模块，期望值由用例给出，
   不依赖任何外部实现当裁判。

### 2.2 非目标

- 不追求与 Android `Paint` / `StaticLayout` 逐像素一致；
- 不照搬 Legado 的类结构、数据库配置结构或全部阅读菜单；
- 本计划不实现漫画、段评、WebDAV、TTS 服务和复杂交互式 HTML；
- 不在首版实现 Rust 字体发现、系统字体 fallback、emoji 光栅化和 GPU glyph atlas；
- 不以“Rust 代码比例”作为性能指标，只看端到端延迟、帧耗时、内存与排版正确性。

## 3. 与 Legado 的关系

书源引擎必须挂 Legado 当裁判，是因为**正确性的定义在生态里**：书源是照着上游的怪癖
写的，差一个字符就抓不到正文。排版不是这回事——一行断在哪、标点怎么悬挂，正确性由
中文排版惯例和肉眼定义，不由某个 App 的实现定义。所以这一块**不建对照 harness、
不做差分**，也不需要为它准备裁判环境。

### 3.1 必须对齐的（数据兼容，不是排版）

| 能力 | 口径 | 为什么 |
|---|---|---|
| 阅读位置 | `chapterIndex + UTF-16 code unit offset` | Phase 4 要导入 legado 备份的进度，语义不一致就落错地方 |
| 正文文本约定 | 段首 `"　　"` 是正文真实字符；`<img src="...">` 原样留在正文 | 正文由 pipeline 产出、已按上游对齐，排版只是消费方，见 §5.1 |
| **章节标题** | 标题是排版内容的**第一个块**，它的字符**计进** `durChapterPos` | 上游同款：`ReadBook.kt:1725,1739` 的 `bodyPosition = durChapterPos - titleLength`。M2a 落地，理由与代价见 §5.1 |

这两条是**已经存在的契约**，排版不得擅自改口径；要改就是破坏性变更，走迁移。

### 3.2 只作参考，不作裁判

`ZhLayout`、`PunctuationCompress`、`HangingLineWidth`、`LineColumnLayout` 是成熟的中文
排版实现，拿它们当**算法与语料的来源**：禁则字符表、压缩规则、悬挂宽度的算法、容易
出错的边界样本。Rubato 的规则写成独立的纯函数 Rust 模块，正确性由自己的用例和肉眼
验收定，不比对上游的输出。抄语料时逐条标注来源行号，便于回头查算法。

### 3.3 排版怎么判对错

- **结构性正确**——不丢字、不重字、页范围单调连续、每步必须推进、锚点可解析。
  这类问题有唯一正确答案，Rust 纯函数测试与 property test 自动判，见 §9.1；
- **排版规则**——禁则、悬挂、压缩、两端对齐、页底对齐，每条规则一组语料 + 人写的
  期望断行结果，见 §9.1。期望值来自中文排版惯例，不来自上游输出；
- **视觉**——各平台自己的 golden 截图，人工过一遍，见 §9.2。不把 Android 截图当
  macOS/Windows 的裁判。

不追求与 Android `Paint`/`StaticLayout` 逐像素或逐页一致：既做不到（字体度量不同），
也没意义（用户不会拿两个 App 对着数页码）。

## 4. 总体架构

```text
Rust pipeline/store
        │ chapter_content
        ▼
reader-doc (Rust)
  ChapterDocument + SourceMap + canonical UTF-16 anchors
        │ PrepareChapterResult（批量）
        ▼
ParagraphBackend (Flutter)
  创建/缓存 ui.Paragraph，测量文字和图片
        │ MeasureBatch（批量）
        ▼
reader-layout (Rust)
  PageComposer + AnchorResolver + LayoutCache
        │ PagePlanBatch（批量/可流式先回当前页）
        ▼
ReaderSurface / ReaderPainter (Flutter)
  clip + drawParagraph + drawImage + overlay
```

### 4.1 Rust crate 边界

计划新增两个 workspace crate，运行态装配仍由 `engine` 与 `ffi` 承担：

```text
rust/crates/reader-doc/
  正文规范化、块模型、render/source offset 映射、稳定 ID

rust/crates/reader-layout/
  测量输入模型、分页器、锚点恢复、缓存键、纯函数测试

rust/crates/engine/
  章节获取、layout session、取消、前后章预取、缓存预算

app/rust/src/api/
  FRB DTO 与薄转换，不放分页业务
```

依赖方向：

```text
rubato-core ← reader-doc ← reader-layout ← engine ← ffi/app-rust
```

`reader-layout` 不依赖 Flutter、FRB、数据库和网络。测试可传固定测量结果，不需要启动 UI。

### 4.2 Flutter 边界

```text
app/lib/reader/
  layout_wire.dart              三段跨 FFI 线格式的 Dart 这一半(块表/行度量/页表)
  paragraph_backend.dart        创建、测量、缓存与释放 Paragraph;几何键与绘制键分家
  reader_layout_controller.dart session/代号/分批测量/装页/翻页/进度/预取
  reader_settings.dart          阅读设置的值与落库(几何位/绘制位分家)
  reader_settings_sheet.dart    设置面板(滑块松手才提交)
  reader_surface.dart           CustomPaint 容器 + 无障碍标签
  reader_painter.dart           只画 PagePlan 引用的行(以及选择/批注/命中那几层标记)
  reader_hit_test.dart          坐标 ↔ 章内 source 下标(M4a)
  reader_selection.dart         选区、标记与选择那件事的全部逻辑(M4b)
  reader_selection_layer.dart   手势、手柄与工具条(M4b/M4c)
  reader_highlights.dart        批注表、样式编解码、内存与落库两步走(M4c)
  reader_search.dart            章内搜索(M4d)
  page_transition.dart          翻页动画:只摆已经装好的相邻两页(M4e)
  toc_sheet.dart                目录抽屉(新旧两条路共用)
  legacy_paginator.dart         被替换掉的那份分页,feature flag 的另一支 + 下线判据的对照
```

M1 落地时与上面的草图有三处出入：`document_adapter.dart` 没有独立成文件——
Rust 那侧交出来的是「正文 + 块表」，Dart 按块表切片即可，没有需要适配的 DTO 结构；
`page_transition.dart` 留到 M4（**M4e 已落地**，见那一片）；
`reader_controller.dart` 的实际名字是 `reader_layout_controller.dart`。

Flutter 持有不可序列化的 `ParagraphHandle`。Rust 只看到稳定 `block_id` 和度量数据，
`PagePlan` 再以 `block_id` 引用 Dart 缓存；Rust 不持有 Dart 对象句柄。

## 5. 核心数据契约

下面是语义草图，不是最终 FRB 代码。M0 必须先锁定版本和不变量再生成桥代码。

### 5.1 正文与位置

```rust
struct ChapterDocument {
    schema_version: u16,
    revision: u64,
    chapter_id: String,
    source_utf16_len: u32,
    blocks: Vec<DocumentBlock>,
    source_map: SourceMap,
}

enum DocumentBlock {
    Title(TextBlock),
    Paragraph(TextBlock),
    Image(ImageBlock),
    Divider(DividerBlock),
    Spacer(SpacerBlock),
}

struct ReadingAnchor {
    chapter_index: i32,
    source_utf16_offset: u32,
    affinity: AnchorAffinity,
}
```

所有持久化位置使用 **UTF-16 code unit offset**。Rust 内部不能把它直接当 UTF-8 byte index；
`reader-doc` 负责构建稀疏索引并完成 UTF-8 byte、render UTF-16、source UTF-16 三者转换。

**缩进由字符承载，不是虚拟内容。** `html-format` 已按 Legado `HtmlFormatter.format`
在正文里插入 `"　　"`（`rust/crates/html-format/src/lib.rs:44`）；Legado 的
`ReadBookConfig.paragraphIndent` 只是「行首前 N 个字符算缩进」的标记，不再插一次
（`TextChapterLayout.kt:1027,1243`）。所以缩进字符是 source 文本的真实字符，计入
`durChapterPos`。`reader-doc` 不得剥离它，排版侧只按 `indent_len` 标注前 N 个字符
参与缩进宽度与两端对齐计算。改成样式缩进属于**破坏性口径变更**：要同时改 §3.1 的
对齐口径、写进度迁移，并接受 legado 备份导入的进度会偏 2 个 code unit。

**正文里的图片是字面标签。** `format_keep_img` 把 `<img src="...">` 原样留在正文
（`rust/crates/pipeline/src/web_book.rs:1345`），这些字符同样占 source offset。契约：
`ImageBlock` 的 source range 覆盖整个 `<img ...>` 标签，其 render 文本为空；加载中、
加载失败与重试都不得改变该 range，占位与错误态只影响 placement 的高度。

**M2b 落地**（2026-09-01，`reader_doc::img` + `ChapterDocument::cut`）在这条契约上补了
三点，都是实现时才浮出来的：

1. **认标签的口径对齐 `AppPattern.imgPattern`**（`judge/engine/.../constant/AppPattern.kt:14`），
   手写扫描、不拉正则库（`reader-doc` 的依赖表是空的）。这不是「排版挂裁判」——
   「正文里哪一串字符算图片」决定块怎么切、`durChapterPos` 怎么分，与 §3.1 的
   「正文文本约定」同类。认不出来的 `<img`（没有 `src="`、没闭合、跨行）**按字面文本走**，
   与 M1 一样，不为「长得像图片但不是」发明第三种块；
2. **图片块的 source range 比标签宽一点**：紧挨着它、**全是空白**的那一截（段首缩进
   `　　`、行尾空白、块间换行）并进图片块。契约说的是「覆盖整个标签」，这里是覆盖它
   **和它两边的空白**。理由：`<p><img …></p>` 经 `format_keep_img` 出来是
   `　　<img src="…">`，把那两个全角空格切成独立段落块，屏幕上就是图片**上方多一条空行**。
   裁判是同一个处置（`TextChapterLayout.kt:504` 的 `if (textBefore.isNotBlank())` 直接
   丢掉空白前缀），差别只在它是「丢掉」、这里是「并进图片的 source 区间」——
   块必须无缝覆盖全章，丢掉就出洞了（§5.5）；
3. **行内的图片把那一行切成几块**（前文一块、图片一块、后文一块）。M2b 的标题是
   「块级图片」，而这条让「块级」成为**所有**图片的形态：不做裁判那套 `imageStyle`
   的文字内嵌（`srcReplaceChar` 占位符那条路），也就不必在 render 文本里插入
   source 里没有的字符 —— 那是 §5.1 明令要走 `SourceMap` 的情形，而 `SourceMap`
   到 M3 才长出来。

**章节标题是接进正文的真字符，不是虚拟块**（M2a，2026-09-01）。`open_chapter`
从目录取到标题，按 `reader_doc::document_text` 拼成 `标题 + "\n" + 正文` 之后再切块，
第一个块的 `kind` 是 `Title`。两条理由，一条对内一条对外：

1. **虚拟块会当场违反分页不变量。** 虚拟块的 source 区间是空的（`[0, 0)`），而 §5.5
   要求「任何非空页至少推进 1 个 code unit」。标题独占一页时那一页就是 `[0, 0)`，
   后一页也从 0 起 —— 装页器会把它并掉（`merged_pages` 加一），标题页就此消失；
2. **裁判的 `durChapterPos` 本来就含标题**（§3.1 新增的那一行）。所以契约是**更贴近**
   了，不是漂了。

代价说清楚：M1 存下的进度是**不含标题**的正文下标，读回来会往前偏一个标题的长度
（十几个 code unit，不到一行）。这是一次性的，不随设置反复漂 —— Rubato **不做**
「隐藏标题」开关，正是为了不引入上游 `resolveHighlightChapterPosition` 那套补偿
（标题一藏，全章 offset 平移）。要做那个开关时，补偿是它自己的账。

标题里的换行一律收成空格：标题恒为**一个块**，省掉「标题占几块」这个要在两侧同步的数。

除以上三类外，render 与 source 文本的其余差异一律经 `SourceMap` 映射，不能给正文
直接加字符后继续使用原下标。

### 5.2 布局键

```rust
/// 几何键：决定行度量、PagePlan 与锚点
struct LayoutKey {
    document_revision: u64,
    viewport_width: f32,
    viewport_height: f32,
    device_pixel_ratio: f32,
    text_scale_key: u64,
    font_fingerprint: u64,
    geometry_style_revision: u64, // 字号、行距、段距、缩进、边距
    locale_key: u64,
    typography_flags: u32,
}

/// 绘制键：只决定 Paragraph 要不要重建，不参与分页
struct PaintKey {
    foreground_color: u32,
    background_color: u32,
    decoration_revision: u64,
}
```

缓存命中必须同时满足正文、视口、字体、系统文字缩放、locale 与排版选项一致。
浮点值进入键前统一量化，禁止直接依赖 `f32` 位级偶然相等。

两个键必须分开：`ui.Paragraph` 的颜色烘焙在 `ParagraphBuilder` 里，换主题/换前景色
必须重建 Paragraph，但几何完全没变——行度量、`PagePlan` 与锚点全部复用，不重新分页、
不跨 FFI。把颜色混进 `LayoutKey` 会让换个主题就整章重排。

**M1 落地**：两个键都落在 **Dart**（`ParagraphBackend.configure`，返回
`none`/`repaint`/`relayout` 三档），不在 Rust。理由是它们的消费者只有一个 ——
Dart 那份 Paragraph 缓存；而 Rust 那侧判「结果过没过期」用的是 `generation`，
任何几何输入变化都必然先 bump 它，所以 `LayoutKey` 的全部输入都被它盖住了。
把同一把键在两侧各实现一遍（包括浮点量化），换来的只是两份会各自漂的量化函数。
`document_revision` 也没进几何键：换 revision 必然伴随换 layout session，
那一步 controller 已经显式作废整批；让它也进键，结果是每次换章白白重排两遍。

### 5.3 测量批次

Flutter 每个 block 只创建并 layout 一次 Paragraph，随后一次性回传：

```rust
struct BlockMeasure {
    block_id: u64,
    width: f32,
    height: f32,
    lines: Vec<LineMeasure>,
}

struct LineMeasure {
    render_start_utf16: u32,
    render_end_utf16: u32,
    top: f32,
    baseline: f32,
    height: f32,
    width: f32,
    hard_break: bool,
}
```

`LineMeasure` 在 `BlockMeasure.lines` 里的下标就是 `Placement::Lines` 引用的行号，
两侧必须同序；第二阶段 Rust 自行断行时，行号改为指向 Rust 产出的行表，协议不变。

实现时不为成千上万条 line 建跨 FFI 小对象。重复字段分别打包进 typed array 与位图标志，
并带 `schema_version`、block offset table 和长度校验。（FRB 2.13 对 `Vec<u8>` 有 strict
typed-list 通道；其余 prim 类型若映射不理想，退路是全部打进一个 `Uint8List` 用 `ByteData`
解，不构成风险，不值得为它单开一次 codegen 验证。）

**M1 落地**（2026-09-01）：走的就是「全部打进一个 `Uint8List`」那条退路，三段缓冲各自
自描述，格式写在 Rust 侧的模块注释里、由两侧的用例逐字节钉住：块表
`reader_doc::wire`、行度量 `reader_layout::measure`、页表 `reader_layout::pages_wire`；
Dart 那一半在 `app/lib/reader/layout_wire.dart`。与上面的草图有两处出入：

- **没有 block offset table**。没有任何调用方要随机访问某一块，顺序读 + 长度严校验
  已经能把「Dart 少写了一段」当场判死。真要随机访问时是一次版本号的事；
- **`LineMeasure` 只有四个位**（`render_start`/`render_end`/`top`/`height`）。
  `width`、`baseline`、`hard_break` 在 M1 一个都没用上 —— 按 M0 自己的教训
  （「三天里实测已经改了两次协议，先写字段表必然对不上」），等 M3 的两端对齐真要
  `width` 时再加位、升版本。

**M2a 把两段线格式升到 v2**（2026-09-01）。两处改动同属一次「标题/正文样式分离」：

- **块表多了 `kind` 位**（`reader_doc::BlockKind`：0 段落 / 1 标题）。M2b 的图片是
  在这里续一个值，不再升版本；Dart 读到不认识的种类**整份拒绝** —— 拿段落的样式去量
  一个图片块，量出来是一段能画、但完全不对的东西，比不画更难查；
- **段距从「装页参数」变成「每块一个数」**：行度量的块头里 v1 那个 `reserved` 换成了
  `space_before`，`ComposeParams.block_spacing` 随之删掉。理由是标题上间距、标题下间距
  与段距是**三个不同的数**，而选哪个取决于「前一块和这一块各是什么种类」—— 那是样式
  问题，样式在 Dart。装页器只管加这一个算好的数，并**在页顶折叠**它（不折叠的话每页
  顶上都空一个段距，既不是排版意图，还让每页少装一行）。装页器因此不需要认识「标题」，
  也就不会因为多一种块而改一次。

两侧的版本号一起走（`kLayoutSchemaVersion = 2`）：它们本来就是同一次改动的两面。

**M2b 把块表升到 v3**（2026-09-01）：`kind` 多了图片（值 2），记录里跟着多了
`src_start` / `src_end` —— 图片 URL（含 `,{option}`）在正文里的区间，记录长度
20 → 28 字节。为什么记这一对而不是让 Dart 自己从 `<img src="…">` 里抠：那是**第二个
解析器**，而两个解析器迟早各自漂（带 `,{"width":"50%"}` 的 src 里有引号，抠法一不小心
就断在中间）。Rust 这一侧本来就要认标签才切得出块，顺手把区间记下来。

行度量与页表的**位一个没动**，但版本号跟着升到 3：Dart 那一半用同一个常量校验三段
缓冲，而三段本来就是同一次构建里一起出厂的 —— 分成两个号只多一处要记得同步的地方，
换不来任何能单独换半边的场景。

**取每行的字符区间只能走 `getPositionForOffset`。**（**M3a 推翻了这一句**，改用
`getLineNumberAt`，见本节末尾。以下是 M0 当时只在两条路之间做的选择，留着因为
`getLineBoundary` 那条仍然是错的。）`getBoxesForRange` 返回的是**每行一个 box**
（100 字的段落只回 5 个 box），只给几何、不给字符下标，反推不出行区间。可用的两条路
差距是量级级别的（ms）：

| 单段 UTF-16 | host `getPositionForOffset` | host `getLineBoundary` | **device `getPositionForOffset`** | **device `getLineBoundary`** |
|---|---|---|---|---|
| 10 000 | 1.8 | 12.1 | 11.3 | 153.9 |
| 50 000 | 12.9 | 274.2 | 80.8 | 7 118.7 |
| 200 000 | 104.7 | 4 569.8 | 648.3 | **136 406** |

`getLineBoundary` 单次调用的成本随本 Paragraph 的长度线性增长，逐行调即 O(n²)，真机上
200k 单段要两分多钟。M0 因此选了 `getPositionForOffset(Offset(0, 行顶))`——值已逐行核对
与 `getLineBoundary` 一致。自然写法恰恰是错的那条，写进 `ParagraphBackend` 的注释。

**M3a 换成了第三条：`getLineNumberAt` + 指数外推/二分**（2026-09-01）。换的理由是
**正确性**：`getPositionForOffset(Offset(0, y))` 取的是**视觉左缘**那一点的位置，纯 LTR
下它恰好等于行首，但一行里混进 RTL 时，RTL 段的视觉左缘是它的**逻辑末尾**——一段阿拉伯语
正文整份行区间都会往后偏（实测首行从 5 起，不是 0），`lineText` 因此切错正文，倒挂时
直接 `RangeError`。`getLineNumberAt` 对下标**单调不减**（每行盖着一段连续的**逻辑**区间，
bidi 只在行内重排、不跨行），所以行首就是「行号第一次 ≥ i 的那个下标」，与 bidi 无关。

顺带比原来快得多——同一条 harness（M3a 的探针 ⑧），**参考机**：

| 单段 UTF-16 | 行数 | `getPositionForOffset` | **`getLineNumberAt`（现产品）** |
|---|---|---|---|
| 10 000 | 500 | 14.2 ms | **4.4 ms** |
| 50 000 | 2 500 | 81.0 ms | **10.1 ms** |
| 200 000 | 10 000 | 792.4 ms | **48.6 ms** |

前者每行的成本随**行数**线性（10 000 行时单次已到 79 µs），后者每行是 O(log 行数)。
这一换把 §10 那笔「字号重排压线」的账还了，见那一节。

### 5.4 页面计划

```rust
struct PagePlan {
    page_index: u32,
    source_start_utf16: u32,
    source_end_utf16: u32,
    placements: Vec<Placement>,
}

enum Placement {
    /// 行级引用：第一阶段 line_start..line_end 来自 Paragraph 自己的换行，
    /// 第二阶段来自 Rust 断行；两阶段共用同一形状。
    Lines {
        block_id: u64,
        line_start: u32,
        line_end: u32,
        origin_x: f32,
        origin_y: f32,
    },
    Image { /* image_id + rect + fit */ },
    Decoration { /* divider/highlight/debug overlay */ },
}
```

`Placement` 从第一版就是**行级**的：`PagePlan` 不引用「段落的某个矩形」，而引用
「某个 block 的第 i..j 行」。这是为了让 §7.2 第二阶段（Rust 决定断行）只换行的来源、
不换协议——否则禁则/两端对齐/悬挂一进来，「整段 Paragraph 自己换行 + 按矩形裁剪」
的模型要整体推翻，§2.1 目标 5 就是空话。

绘制后端怎么实现对 Rust 不可见：

- 第一阶段：行来自 Paragraph 自己的换行，backend 为可见行各建一个 Paragraph 并缓存
  （建一次、跨帧复用，翻页热路径只是 `drawParagraph`）。整段 Paragraph 仍然建，
  它负责测量与命中测试，只是不拿它绘制；
- 第二阶段：行由 Rust 决定。禁则、孤行这类只改断点的规则，最省的走法是往 render
  文本插分隔符后重建 Paragraph（source 文本不变，`SourceMap` 吸收），代价是该次
  重排多一次往返；两端对齐、标点悬挂与压缩需要行内逐字 x，只能由 backend 按行
  绘制或逐 column 绘制。

**M0 实测结论**（2026-09-01，harness `app/test/reader_layout_m0_spike.dart`，两处口径：
`host` = flutter_tester + 测试字体 + 软件光栅；`device` = HITV205N，Android 11 / arm64
的电视盒子，profile 模式，作低端参考机）。

一页的绘制耗时（录制 / 含光栅，ms）：

| 单段 UTF-16 | host A:整段clip | host B:按行 | **device A:整段clip** | **device B:按行** |
|---|---|---|---|---|
| 2 000 | 0.007 / 0.493 | 0.008 / 0.493 | 1.026 / 7.885 | **0.340** / 6.752 |
| 50 000 | 0.105 / 0.618 | 0.007 / 0.503 | 23.163 / 38.901 | **0.340** / 6.997 |
| 200 000 | 0.465 / 0.986 | 0.007 / 0.478 | 72.484 / 87.502 | **0.306** / 6.659 |

**结论：第一阶段就按行绘制，不用整段 clip。** host 上两者在真实段落长度下持平，看着像
「整段 clip 够用」；真机把这个结论推翻了——2 000 字单段就差 3 倍，50 000 字单段光录制
就 23 ms，超过一帧预算。按行绘制则与段落长度无关，恒定 0.3 ms。clip 只挡光栅，挡不住
display list 的录制与回放，而录制成本随整段长度线性增长。

这也说明为什么 `Placement` 必须行级：如果协议是「段落 + 裁剪矩形」，这个结论出来时要
改的是协议，不是 backend 的一个分支。

**M2b 没有在 `Placement` 上续 `Image` 这一支**（2026-09-01），虽然上面的草图和 M1
的注释都说要。理由是**装页器不认识图片**：图片块的 render 文本是空的，它在行度量里
天然就是「一条空行（`render_start == render_end == 0`），高度 = 这张图在当前视口里
占多高」，于是页表里它就是一段 `0..1` 的行段，`compose` 一行代码都不用改。

反过来，真要在页表里立一种 `Image`，得先把图片的**宽度**也送进装页器（不然摆不出
矩形）—— 那是给 `LineMeasure` 加 `width` 位，而 M2a 已经把「不为居中先开一个位」
写进账上了（等 M3 的两端对齐真要 `width` 时一起加）。这与 M2a「装页器不认识标题」
是同一条：**块多一种，装页器不改**。画的时候按 `BlockKind` 分岔 —— 绘制侧本来就要
按种类取样式（标题用标题字号），多一个分支不是新引入的形状。

代价说清楚：页表因此**不是自描述的**，读它要配着块表看「这一块是什么种类」。
两张表本来就同生共死（同一个 session、同一个 revision），所以这不新增一处会漂的地方；
真到 M4 的高亮那种「页表里有、块表里没有」的东西进来时，`kind` 位仍在原处等着续号。

**M3d 在 `Placement::Lines` 上加了一个 `line_gap`**（2026-09-01）：页底对齐摊进来的
额外行距，绘制方按 `y = origin_y + line.top + (i - line_start) * line_gap` 画。
它占的是页表线格式里 v1 那个 `reserved` 位，记录长度没变。这是三片里唯一一次**动到
页表**——理由是同一份行度量在不同的页上摊到的数不一样，那就是「这一页怎么摆」的一部分。

**M2c 的滚动模式也走这同一个形状**（2026-09-01）：滚动 = 页高无穷，整章落成一页，
`origin_y` 变成章内绝对坐标。页表、线格式、绘制器一处没改 —— 「不断页」本来就是
「页高够大」的极限情形。所以「行级 `Placement`」这条从 M1 起就下的注，到这里第三次
兑现了（第一次是 M2a 的标题，第二次是 M2b 的图片）。

选择、点击与高亮通过 Paragraph 的位置 API 映射回 render offset，再由 `SourceMap`
转成持久化 source offset。

**M4a 补一条口径**（2026-09-02）：这里的「Paragraph」是**画出来的那份行 Paragraph**，
不是测量用的整段 Paragraph。后者按 `TextAlign.start`、不摊行距、标题不居中量，
拿它反算坐标会偏 —— 理由与判据见 M4a 的出入第 2 条。

### 5.5 offset 的边界与溢出口径

跨端约定，M0 定死，不随实现改。§9.1 的 property test 逐条校验。

**单位与类型**

- 一律 UTF-16 code unit。Dart 的 `String` 下标天然是它；Rust 侧禁止把它当 UTF-8 byte index；
- Rust 用 newtype 包装 `SourceUtf16(u32)` 与 `RenderUtf16(u32)`，两者只能经 `SourceMap`
  转换，不提供 `From`/`as` 隐式互转——混用要在编译期失败，不是运行期查出来；
- 章内长度上限 `u32::MAX`。`reader-doc` 构建时若 `source_utf16_len` 溢出，整章拒绝并报错，
  不静默截断；
- `chapter_index` 用 `i32`，与 store 的 `durChapterIndex` 一致；负值非法。

**合法区间**

- 任何 source offset ∈ `[0, source_utf16_len]`，`== len` 合法（章末锚点）；
- 页范围：`start <= end`；`page[i].end == page[i+1].start`；`page[0].start == 0`；
  `page[last].end == source_utf16_len`。分页对正文的覆盖必须是无缝分割，不允许空隙或重叠；
- 空章：`page_count == 1`，该页 `start == end == 0`——是一页空页，不是零页。

**吸附规则：持久化值不改，消费时才吸附**

- 锚点落在 surrogate pair 中间：向前吸附到 high surrogate；
- 落在 grapheme cluster 中间：显示与选择向前吸附到 cluster 边界；
- 吸附只发生在**消费**锚点时，已落库的值不因吸附被改写。否则每次打开都漂一点，
  几次之后进度就不是用户上次读到的地方。

**推进不变量**

- 任何非空页至少推进 1 个 source code unit；
- 违反时 `debug_assert!` + 上报诊断，release 下强制推进 1 并记 warning。**不照抄**当前
  Dart 实现的 `offset += end > 0 ? end : 1`——那种静默兜底会把「一页装不下一个字」
  这类真 bug 藏成「正文被切碎」，查起来毫无线索。

**M1 落地**改了两处，理由各一条：

- **只上报诊断，不 `debug_assert!`**。`ComposeDiagnostics` 随每次装页回给调用方，
  引擎那侧非零就打 warning。改的原因是 `debug_assert!` 会让这两条路在单测里根本走不到
  （一进去就 panic），而它们恰恰是最需要有用例盯着的两条 —— 现在
  `reader-layout` 有「单行高于整页也必须推进」这条用例，assert 版本写不出来；
- **推进不了的页并进前一页，不是硬推 1 个 code unit**。硬推 1 会让
  `durChapterPos` 指到一个**不是本页首字**的位置，比一页溢出更难查；并页则覆盖与单调
  两条都还在。

**溢出与退化输入**

- 所有 offset 运算走 `checked_add`/`checked_sub`，越界返回 `Err`，不 wrapping 也不 panic；
- 视口宽高或行高为 0/负数：拒绝该次布局、保留上一次结果，不产出 0 高页。

## 6. 运行时与缓存

### 6.1 Layout session

每次打开章节或修改排版条件生成新的 `(session_id, generation)`：

1. Rust 返回 `ChapterDocument`；
2. Flutter 为 block 建 Paragraph，批量测量——**按锚点优先顺序分批**，不是先量完整章
   再分页。真机上整章 layout 是几百毫秒量级（200k 单段 676 ms），先量完整章会把首屏
   拖到秒级；
3. Rust 校验 session/generation 后分页；
4. 先返回包含阅读锚点的当前页及前后页，再补齐其余页索引；
5. 任一新 generation 到来时，旧结果即使完成也不得写入当前状态。

取消以 generation 为准，不能只靠 Future 是否还被 await。Rust 长循环定期检查取消标志；
Flutter 收到过期结果立即释放不再引用的 Paragraph。

### 6.2 缓存层级

- L0：当前、前一、后一页的 `PagePlan` 与绘制资源；
- L1：当前章所有 block 的 Paragraph 与行度量；
- L2：前后章的 `ChapterDocument`，是否保留 Paragraph 由内存预算决定；
- 持久缓存：只缓存正文与阅读进度；首版不持久化平台字体相关 PagePlan。

缓存必须有显式预算与释放路径。正文图片缓存单独计量，不能用图片体积掩盖文字布局内存。

预取有前置依赖，M1 不能直接假设它可用：`engine::chapter_content` 当前是同步阻塞、
会走网络、`CancelToken` 是常假的那一份（`rust/crates/engine/src/lib.rs:611`），也没有
预取队列；正文缓存又与 JS 状态共用 `caches` 且无容量与淘汰（见 data-storage-plan 的 P2）。
L2 预取落地前必须先补：可取消的异步取正文、预取队列与并发上限、正文缓存独立限额。
预取会放大缓存缺陷，这笔账记在本计划头上。

**M1 落地**：三条前置里前两条补上了（`Engine::chapter_content_with` 收令牌；
`Engine::prefetch_chapter` 是一道带并发上限的准入闸），**第三条仍然欠着**。
所以 M1 的预取**只做相邻章** —— 那正是用户下一步本来就会抓的，缓存的增长速度
不会因为预取而变快。跨度更大的预取、以及缓存 `ChapterDocument` 的 L2，
都要等 `caches` 的容量与淘汰落地。

### 6.3 翻页热路径不变量

普通上一页/下一页操作必须满足：

- 提交下一帧前不发起或等待 Rust 调用；
- 不调用 `Paragraph.layout`；
- 不对章节正文执行 `substring`；
- 不重新计算所有页；
- 只更新页索引、绘制相邻已缓存页，并异步补一个新的邻页；
- 进度写库在翻页帧外 debounce 后异步调用 Rust，连续翻页不逐次同步落盘。

跨章时允许异步取正文与排版，但若 L2 预取命中，应直接显示首/末页。

## 7. 中文排版策略

### 7.1 第一阶段

第一阶段使用 Flutter Paragraph 已产生的 line boundary，Rust 只负责把完整行装入页面。
先消除重复 layout，锁住正确性、锚点和生命周期，不在同一改动里重写中文断行。

### 7.2 第二阶段

按以下顺序加入并分别验收：

1. 行首/行尾禁则；**（M3a 实测：ICU 已经免费给了，见本节末尾的「第 1 项」）**
2. 标题不可拆与孤行控制；**（M3b：孤行控制落地；标题不可拆没有落地对象，
   见 M3b 的出入第 1 条）**
3. 首行缩进、段距与标题间距；
4. 正文两端对齐；**（M3c 落地。按行画时它要一手占位符，见那一片）**
5. 标点悬挂；**（M3e 的 spike：不做，门并到 M5）**
6. 标点压缩；**（同上——它要逐字画，是 §10 一页绘制门槛的 2.4 倍）**
7. 页底对齐。**（M3d 落地：只收「差一点点」的那截，判据是每一道行距被撑开多少）**

其中 5–6 项（标点悬挂、标点压缩）**必然**表达不了「整段 Paragraph 自己换行」的模型：
它们要求行内逐字 x，而且会改变一行装得下多少字，即会改断行、直接影响分页结果。
1–2 项（禁则、孤行）可以靠往 render 文本插分隔符后重建 Paragraph 表达，代价是每次
重排多一次往返。按 §5.4 的行级 Placement，这两条路都不改协议，只改 backend 的绘制
方式与往返次数。

第 4 项（两端对齐）**已实测可以直接表达**（2026-09-01，同一 harness）：`ui.Paragraph`
的 `TextAlign.justify` 对无空白的纯 CJK 行确实均摊字距（20 字行余 7 px，逐字左缘位移
0 / 1.47 / 3.32 / 5.16 / 6.63，即每个字间隙 0.368 px），断点与左对齐完全一致，末行不
参与拉伸。也就是说两端对齐既不改断行、也不影响分页，用整段 Paragraph 就够。
真机（HITV205N，真系统字体，汉字 advance 19.999985）复测：逐字位移与 host 完全一致
（0 / 1.47 / 3.32 / 5.16 / 6.63），断点不变。harness 里对这三条留了断言，升级 Flutter
若改了行为会直接失败。

标点悬挂需要 per-glyph 墨水盒，`ui.Paragraph` 不给。出路已在 ADR-002 定死：离屏画单字
扫像素，按 `(字体指纹, 字号, 字符)` 缓存并异步预热。同一份实测还证伪了一个常见假设——
CJK 标点不一定是 1 em（参考机上 `“` 的 advance 只有 15.4），宽度一律实测。

**第 1 项（禁则）不用做**（2026-09-01，M3a 实测，harness
`app/test/reader_layout_zh_rules.dart`）。Skia 的断行走 UAX #14，而 UAX #14 的
CL/EX/NS/IS（禁行首）与 OP（禁行尾）恰好就是中文禁则那张表。把裁判两份表
（`ZhLayout.kt:27-31` 的 `postPanc`/`prePanc`，并上 `PunctuationCompress.kt:32-33` 的
`openChars`/`closeChars`）里的每个字符各造一条语料、**恰好推到自然断点上**，
参考机与 macOS 上 ICU **一个不漏**地挡住了 33 个禁行首 + 18 个禁行尾。

唯一挡不住的是 `<` 与 `>`——它们不是中文标点，UAX #14 归到字母类（AL）。Rubato 的表
**把这两个去掉**：为它们换一条「Rust 自己断行」的路，付的是整条第二阶段，换回来的是
正文里几乎不出现的数学尖括号。

于是这一项能落地的只有一件事：**把它钉住**（升级 Flutter 若改了行为当场失败），
以及一条「无处可断时才豁免」的判据——整行都是禁行首标点时断在哪儿都错，那是标点挤压
（第 6 项）要解的，不是断行。

进入第二阶段前先做独立 spike：批量取得 cluster/glyph box，由 Rust 决定行范围，
再让 Flutter 按行绘制。必须测量重新 shaping 的成本和复杂脚本正确性，
不能直接进入主实现。

Legado `ZhLayout`、`PunctuationCompress`、`HangingLineWidth` 只作为算法与测试样本来源。
Rubato 的规则必须写成独立、纯函数、带语料覆盖的 Rust 模块，不把 Android Paint 状态移入。

## 8. 里程碑

### M0 — 基线、语料与契约

任务：

- [x] 语料按需建 —— 2026-09-01 完成:M1 写第一个测试时加了它真正要用的那一份
      (`app/assets/reader-layout/plain.txt`)。**位置从 `fixtures/reader-layout/` 改到了
      `app/assets/`**:唯一的消费者是 Flutter 集成测试,而它要在真机上读到语料 ——
      Flutter 的 asset 只能取 package 目录里的文件;放 `fixtures/` 下就只有 macOS
      那一头跑得起来。理由与先例记在那边的 README;
- [x] 增加当前实现 benchmark，记录 1k/10k/50k/200k UTF-16 长度 —— 2026-09-01 完成，见 §10；
- [x] 整章分页曲线 —— 见 §10；
- [x] 冷启动首屏、100 次翻页、字号重排的产品路径基线 —— 2026-09-01 随 M1 完成:
      新旧同机同轮,harness `app/integration_test/reader_layout_m1_bench_test.dart`,
      数字见 §10;旧实现的峰值内存不测(它要被删);
- [x] schema v1 **在代码里锁** —— 2026-09-01 随 M1 完成:`reader_doc::SCHEMA_VERSION`
      与 `reader_layout::MEASURE_SCHEMA_VERSION` 各管一段(文档模型与线格式各自演进,
      不互相绑架);`ChapterDocument`/`Block`/`PagePlan`/`Placement` 都是
      `#[non_exhaustive]`。三段线格式的版本对不上一律**整份拒绝**,不按字段猜兼容;
- [x] 锁定位置单位与所有 offset 的边界/溢出检查 —— 2026-09-01 完成，见 §5.5；
- [x] 定稿 §5.1 两条契约 —— 2026-09-01 从代码路径核实：`chapter_content` →
      `format_keep_img` → `format_with(NOT_IMG_HTML, PARAGRAPH_INDENT)`，缩进字符与
      `<img>` 标签都在正文里，`durChapterPos` 口径不变；
- [x] 实测整段 `drawParagraph` + clip 与「按行绘制」A/B —— 2026-09-01 完成，
      结论见 §5.4：**第一阶段就按行绘制**，整段 clip 在真机上不成立；
- [x] 实测 `ui.Paragraph` 对无空白 CJK 行的 justify 行为 —— 2026-09-01 完成，结论见
      §7.2：Skia 对纯 CJK 行均摊字距、不改断行，两端对齐不需要逐字绘制；
- [x] 上面两条在 Android 参考机上复测 —— 2026-09-01 完成：justify 行为与 host 一致；
      绘制那条**结论被真机推翻**，改为第一阶段就按行绘制（§5.4）；
- [x] 参考环境 —— 低端参考机 HITV205N（Android 11 / arm64 / 电视盒子，作悲观下限），
      桌面参考 macOS；Windows 无稳定跑法，记 best-effort；
- [x] 写 ADR —— 2026-09-01 完成，见 §13（ADR-001 不做 Rust glyph renderer；
      ADR-002 悬挂墨水盒走离屏扫像素）。

验收：基线结果入库、fixture 可复算、协议评审完成；此阶段不替换产品阅读页。

### M1 — 纯文本混合排版 MVP

**状态：2026-09-01 完成**。产品阅读页已经换成新实现，旧那份留在 feature flag 后面。

任务：

- [x] 新建 `reader-doc` 与 `reader-layout` crate —— 两个都不依赖 Flutter/FRB/数据库/网络，
      单测直接喂字符串与行度量；
- [x] 实现段落切分、稳定 block ID 和 SourceMap；**UTF-16 稀疏索引没建**，
      理由见下面「与计划的出入」第 1 条；
- [x] 实现 Flutter `ParagraphBackend`，每个 block/layout key 只 layout 一次
      （`app/lib/reader/paragraph_backend.dart`，判据在 `app/test/` 与 M1 集成测试 ③）；
- [x] 实现批量 MeasureBatch 与 PageComposer；
- [x] 实现 `PagePlan` 绘制、当前/前/后页缓存（`retainLines` 的 L0 窗口）；
- [x] 接管当前阅读页的翻页、跨章、进度保存与重排恢复；
- [x] engine 侧补可取消的异步取正文与前后章预取（`Engine::chapter_content_with` /
      `Engine::prefetch_chapter`）；**预取队列**是一道准入闸而不是自建线程池，
      见下面第 5 条；
- [x] 保留旧实现为短期 feature flag（`--dart-define=RUBATO_LEGACY_READER=true`，
      `app/lib/pages/reader_page_legacy.dart` + `app/lib/reader/legacy_paginator.dart`）；
      下线判据实测结果见下面第 7 条；
- [x] 加入 session/generation 过期结果防护和资源释放测试（两侧都判：
      Dart 的代号检查 + `engine::reader` 的「见过的最大代号」）。

验收：

- [x] **当前阅读集成测试全部通过** —— `app_test.dart`（书架 → 搜索 → 加书架 → 阅读 →
      翻页 → 进度)在 Android 与 macOS 各跑过一遍；正文的 finder 从「长 `Text`」
      换成了 `ReaderSurface` 挂的无障碍标签(正文现在是 `CustomPaint` 画的,树里没有
      装正文的 `Text`；读屏看到的也是这一份，所以这不是为测试加的钩子);
- [x] **纯文本分页前后 UTF-16 正文连续且完全覆盖** —— M1 集成测试 ①：三种视口下
      逐页拼回去必须等于正文去掉块间换行，加上 §5.5 的全部页范围不变量；
      Rust 侧另有 property test（随机块序 + 随机行度量 300 轮）；
- [x] **页面热翻在提交下一帧前不跨 FFI、不重新 layout；进度仅在帧外合并写入** ——
      M1 集成测试 ②：`nextPage()` 前后 `measureCount` 与 `lineBuildCount` 都不变
      （它是同步的，跑完之前不可能有异步 Rust 调用落地）；相邻页的行 Paragraph 由
      `warmWindow()` 在 post-frame 回调里备好；进度走 400 ms debounce，换章与退页强制 flush；
- [x] **200k fixture 不出现随页数重复 layout 的增长曲线** —— M1 集成测试 ③：
      504 页、测量次数**等于块数**；曲线见 §10（整章补齐 20k→200k 是 124→742 ms，近似线性）；
- [x] **Android、macOS 各完成一次产品路径验收** —— 见上；Windows 按 M0 的结论 best-effort，
      本轮没测。

另外加了一条计划里没写、但少了它整套判据都可能全绿而屏幕是空的：**正文真的落在画布上**
（M1 集成测试 ⑦）。前面几条判的都是「页表对不对」——页表对、画不出来是两回事，
按行画那条路只要把 `origin_y` 算飞，页表照样全绿。这一条把 `ReaderPainter` 画进一张位图、
数着墨像素，并检查最下面那一行的墨迹落在视口里、且用满了大半页。

#### M1 与计划的出入（八条，各带理由）

1. **UTF-16 稀疏索引没建**。索引的用处是「按 UTF-16 下标去切 Rust 侧的文本」，
   而 M1 的 Rust 一次都不切文本：正文只跨一次 FFI 交给 Dart，块边界在切块那一遍
   顺手算出，锚点定位走的是块/页的 source 区间。造一个没有消费者的索引，
   只会让人以为它已经在做事。M2 的图片或 M3 的 render 文本改写真要它时再建；
2. **`LayoutKey`/`PaintKey` 落在 Dart，不在 Rust**。理由见 §5.2 的「M1 落地」；
3. **行度量线格式没有 block offset table，`LineMeasure` 只留四个位**。见 §5.3；
4. **装页的不变量违例只上报诊断、不 `debug_assert!`；推进不了的页并进前一页**。见 §5.5；
5. **预取是准入闸，不是自建队列**。`Engine::prefetch_chapter` 只判「这一章要不要抓、
   现在准不准抓」（已缓存 / 章号越界 / 在飞 / 超并发上限一律立刻返回 false），
   执行体是 FRB 自己的 worker 池 —— Dart 发一个不 await 的调用。自己再起一套线程池，
   换来的只是多一条要管生命周期的线程，而并发上限那件事一个 `HashSet` 的容量就表达完了。
   **只预取相邻章**也是有意的：`caches` 至今没有容量与淘汰（data-storage-plan 的 P2），
   而相邻章正是用户下一步本来就会抓的，所以缓存的增长速度不会因为预取而变快。
   跨度更大的预取要等那条债还上 —— §6.2 的三条前置里，第三条**仍然欠着**；
6. **一次冷开章是三次往返，不是两次**。`prepareChapter` 一次 + 装页两次
   （先前缀出首屏、后全量补页表）。§1 的「两次」是对**一次重排**说的，而重排
   （字号、转屏、换主题）确实还是两次 —— 换章是另一件事。首屏那次的第二遍装页
   是 §6.1 第 4 步明写要的，不做就得等整章测量完才出第一屏（参考机上是几百毫秒）；
7. **下线判据是「差异逐条有理由」，不是「集合一致」**。实测（M1 集成测试最后一条，
   Android 与 macOS 都跑）：新旧页首前一两页相同，之后**新的永远不比旧的靠后**，
   差距每页累积约一行。理由只有一条，对每一页都成立 —— 旧实现按
   `getPositionForOffset(视口右下角)` 切页，那个位置落在**跨过页底的那一行**里，
   于是这一行整条算进本页、底部被裁掉一截；新实现按行装页，装不下的留给下一页。
   判据因此写成两条可判的断言：**旧的每一页(末页除外)都超出视口高度，新的每一页都不超**。
   这不是回归，是把「末行被裁一半」修掉了；
8. **`Placement` 在 M1 只有「行段」一个变体**，但线格式里**带了 `kind` 位**。
   M2 的图片、M4 的高亮进来时是续一个 kind 值，不是改协议；Dart 读到不认识的 kind
   **整份拒绝**（半懂半不懂地画出来比不画更难查）。

#### M1 欠下的账

- **`caches` 的容量与淘汰**（data-storage-plan 的 P2）仍然欠着。M1 靠「只预取相邻章」
  把它按住，不是解决它。§6.2 的 L2 文档预取要等它；
- ~~**整段 Paragraph 测完就释放**（只留行度量）。M4 的命中测试要整段 Paragraph 时得按需重建~~
  （**这笔取舍从来没有被兑现过**，见 §8.6）。M4a 落地时命中测试走的是
  **按行 Paragraph + 缓存**（`reader_hit_test.dart` 的 `_lineParagraph`，与绘制同一个入口），
  整段 Paragraph 一次都没重建过 —— 而且**必须**这样：整段 Paragraph 是 `TextAlign.start`，
  拿它反算标题那一行会偏出几十像素（M4 判据 ② 判的正是这个）。所以这条账记错了方向：
  它当时以为按行绘制是命中测试的负担，实际上按行绘制是命中测试的**前提**；
- ~~**正文里的 `<img>` 标签在 M1 按字面文本走**~~（**M2b 已兑现**）。
  §5.1 关于 `ImageBlock` source range 的契约在 M2 兑现；它占的 source 位置本来就在正文里，
  所以现在不需要为「还没实现图片」额外记账；
- ~~**`ReaderSurface` 的无障碍标签是逐页拼的**（O(本页)）~~（**已还**，见 §8.6）。
  收法是 `ReaderPainter.semanticsBuilder`：读屏节点**按段一个**，而且只在读屏真的开着时
  才拼 —— 原来那个 `Semantics(label:)` 的字符串是 `build` 的入参，滚动时**每一帧**都要
  拼一遍整页正文，而没人在听。

### M2 — 样式、图片与阅读设置

M2 的六条任务**拆成三片**做，因为第一片要动块模型与线格式，后两片都建在它上面：

| 片 | 内容 | 状态 |
|---|---|---|
| **M2a** | 标题/正文样式分离、阅读设置（字体/字号/字重/行距/段距/边距/配色）、换主题只重绘、重排锚点恢复 | **2026-09-01 完成** |
| **M2b** | 块级图片：`ImageBlock`、测量占位、加载成功/失败回流、超高图片策略 | **2026-09-01 完成** |
| **M2c** | 滚动模式与分页共用 `ChapterDocument`；滚动下 `durChapterPos` 的口径 | **2026-09-01 完成** |

#### M2a — 样式与阅读设置（已完成）

- [x] 标题/正文样式分离 —— 块的种类在**切块那一遍**定死（`reader_doc::BlockKind`，
      §5.1），样式按种类分家（`ReaderTextStyle` 的 `titleScale` / `titleWeight` /
      标题上下间距）。换字号不会让一个块从标题变成段落；
- [x] 字体、字号、字重、文字缩放、行距、段距、边距 —— `ReaderSettings`
      （`app/lib/reader/reader_settings.dart`），落库走引擎的通用 kv `reader:settings`。
      **「缩进」不在其中**，理由见下面「出入」第 3 条；
- [x] 主题、背景、前景色只重建 Paragraph，不重算分页 —— 机制在 M1 就落了
      （§5.2 的几何键/绘制键分家在 `ParagraphBackend.configure`），M2a 补的是
      **判据**：M2 集成测试 ④ 判「换配色之后 `measureCount` 不变、页表是**同一个对象**、
      页号与 `durChapterPos` 都不动」；
- [x] 横竖屏、窗口缩放、安全区变化后的锚点恢复 —— M2 集成测试 ⑤：翻两页之后
      换字号、再转屏，每一步都要求读者仍停在原来那段正文上；
- [x] 设置面板（`reader_settings_sheet.dart`）与它的两条判据：窄屏不溢出、
      **滑块松手才提交**。

验收（M2a）：

- [x] **七条判据在 macOS 与 Android 参考机各过一遍** ——
      `app/integration_test/reader_layout_m2_test.dart`，离线可复算（同 M1，
      经 `prepareChapterText` 进 Rust，不挂网络）；
- [x] **M1 的八条判据 + 产品路径 `app_test.dart` 不回归** —— 两台各跑过；
- [x] Rust 侧新增用例：`reader-doc` 4 条（标题成块 / 换行收成空格 / 空白标题退回 /
      标题与正文都空）、`reader-layout` 4 条（页顶折叠 / 每页页顶都折叠 /
      标题靠自己的块前间距拉开 / 间距非法当场拒绝）、`engine` 3 条
      （产品路径接标题 / 判据口不接标题 / 设置原样存取）；
- [x] Dart 侧新增用例：`reader_settings_test.dart` 8 条、
      `reader_settings_sheet_test.dart` 4 条、`reader_paragraph_backend_test.dart`
      多出 6 条（样式分家、行按种类建、块前间距三选一）。

##### M2a 与计划的出入（五条，各带理由）

1. **标题接进正文的真字符，不是虚拟块**，`durChapterPos` 的原点因此含标题。
   两条理由与那笔代价写在 §5.1；契约表 §3.1 也跟着加了一行 —— 这是**向裁判靠拢**，
   不是漂移；
2. **段距从 `ComposeParams` 挪进了行度量线格式**（每块一个 `space_before`，
   §5.3）。装页器因此不认识「标题」，M2b 的图片进来时它也不用改；
3. **「缩进」没做成设置**，这是有意的。§5.1 已经定死：段首缩进由正文里的全角空格
   承载，是 `durChapterPos` 里的真字符。做成可调的样式缩进要么改正文字符、要么让
   两种缩进并存，两条都是 §5.1 点名的**破坏性口径变更** —— 那是一次带进度迁移的
   改动，不是一个滑块；
4. **字体只给通用族名**（`serif` / `sans-serif` / `monospace`）。§2.2 把「Rust 字体发现
   与系统 fallback」列为非目标，所以这里拿不到「装了哪些字体」。Android 认这三个族名；
   桌面上认不出来的会**静默退回系统默认** —— 不报错也不崩，只是看不出变化。
   要列出真实字体是一条要走平台通道的独立任务，记在下面的账上；
5. **滑块松手才提交**（`onChangeEnd`），拖动中只更新本地预览数字。按 `onChanged`
   提交等于拖一下整章重排几十遍：代号机制会把过期的那些丢掉，但活儿已经干了。
   参考机上一次重排到首屏是百毫秒量级（§10）。

##### M2a 欠下的账

- **字号重排到首屏那一档仍然压线**（§10：50k 那档 169 ms vs 门槛 150 ms）。M1 把它
  记在「M2 的账」上，M2a **没有还**。真因不是装配问题：重排的锚点在读者当前位置，
  锚点**之前**的块必须先量完才知道页号。当时设想的解法是让装页器支持从锚点块起装
  **后缀**（先出锚点页、再往回补页号）。**M2c 把它想清楚了：那个方案做出来更差**
  （会拿 100 ms 的等待换一次正文跳动，或者留下永久的短页，还要松掉 §5.5 最硬的
  那条不变量）—— 完整的账写在 §10 的「那笔『字号重排压线』的账」，结论是不做，
  门槛留到有采样时再定；
- **看不到真实字体列表**（出入第 4 条）。要一条平台通道枚举系统字体，才谈得上
  「选字体」而不是「选族名」；
- ~~**标题没有居中**~~（**M3c 已还**）。居中要按行的实际宽度摆 `origin_x`，而
  `LineMeasure` 在 M1 刻意没留 `width`（§5.3）—— M3c 的两端对齐真要 `width` 时
  一起加了，判据是 M3 真机那套的 ⑭。「不为居中先开一个位」这一手是对的。

#### M2b — 块级图片（已完成）

- [x] `ImageBlock`：source range 覆盖整个 `<img ...>` 标签，render 文本为空
      （`reader_doc::img` 认标签、`ChapterDocument::cut` 切块；三条补充口径
      —— 认标签的口径、空白并进图片、行内图片切成几块 —— 写在 §5.1 的「M2b 落地」）；
- [x] 块级图片测量、占位、加载成功/失败回流和超高图片策略
      —— 尺寸是一个纯函数 `imageBox`（`app/lib/reader/reader_images.dart`），
      三档各有确定高度（占位 = 视口高的 0.4、失败 = 72、取到 = contain 到视口且不放大）。
      **没有「高度未知」这一档**：未知就排不出页，而排不出页就没有首屏；
- [x] 页表里的 kind **没有续号**，理由与代价写在 §5.4 的「M2b 没有在 `Placement` 上
      续 `Image` 这一支」。块表的 `kind` 续了（值 2），线格式升到 v3（§5.3）。

取图走引擎（`Engine::chapter_image` → `pipeline::get_image` →
`AnalyzeUrl::get_byte_array`），对齐裁判的 `BookHelp.saveImage`
（`AnalyzeUrl(src, source = bookSource).getByteArrayAwait()`）。**不是** Dart 侧
`Image.network`：书源的图片十之八九要带着这个源自己的 header（Referer / UA）
与 cookie 才取得下来，而那两样都在引擎这条链路上。解码在 Dart（那边才有平台的
图片解码器与 `ui.Image`）。取图那一步在判据里由构造参数换成喂字节的桩 ——
与 M1/M2a 同规矩：判据离线可复算。

验收（M2b）：

- [x] **四条判据在 macOS 与 Android 参考机各过一遍**
      （`app/integration_test/reader_layout_m2_test.dart` 的 ⑧～⑪：img 自成一块且进页表
      与读屏标签 / 图片到齐只重装页不重新测量且读者不被扔走 / 超高图片压到视口以内且
      诊断干净 / 取不到的图按失败档占位不把正文拦住）。参考机上**连跑两遍**，
      特意盯 ⑨ ——「取图慢 → 重装页落在测量还没铺满的时候」那条时序只有慢机器跑得出来，
      两遍都稳；
- [x] M1 的八条判据不回归（两台各一遍）—— 两条改了口径，各带理由，见「出入」第 5 条；
- [x] 产品路径 `app_test.dart` 两台不回归；`engine_test.dart`（macOS）不回归；
- [x] Rust 侧新增用例：`reader-doc` 的 `img` 11 条（认标签的每一种形状）+ 切块 8 条、
      `reader-layout` 5 条（图片按一条空行装页 / 满页高的图自己占一页 / 连着的图不被并掉 /
      只有一张图的章 / 图片块行区间越界判死）、`engine` 2 条；
- [x] Dart 侧新增用例：`reader_images_test.dart` 10 条、线格式与块前间距各多一条。

##### M2b 与计划的出入（五条，各带理由）

1. **页表里没有 `Placement::Image`**（§5.4）。装页器不认识图片 —— 图片块的 render
   文本是空的，它天然就是「一条空行，高度 = 这张图占多高」。要立一种 `Image`，
   反而得先给 `LineMeasure` 加 `width` 位，而那是 M2a 记在账上「等 M3 一起加」的；
2. **图片块的 source range 比标签宽一点**：紧挨着它、全是空白的那一截（段首缩进、
   行尾空白、块间换行）并进它。不并的话图片上方白空一条空行；不能像裁判那样直接丢，
   因为块必须无缝覆盖全章（§5.5）。展开在 §5.1；
3. **回流是「只重装页」，不是「重新测量」**。图片取到之后变的只有几张图占多高，
   那些文字的行度量一位没变 —— 所以 `_recompose` 拿着 `_lineBoxes` 重新装一次页就够。
   整章重新测量在参考机上是百毫秒量级（§10），一章十几张图就是十几遍，那会让
   「图片陆续到齐」变成「正文一直在抖」。判据 ⑨ 判的就是这条：`measureCount` 不许变；
4. **图片字节不落库**。`caches` 是 TEXT 的通用 kv，而它的容量与淘汰本身还欠着
   （data-storage-plan 的 P2）—— 给它塞 base64 的图片只会把那笔债做大。当前缓存是
   Dart 侧按 URL 记的一份内存表，跟着章一起丢。记在下面的账上；
5. **M1 的两条判据改了口径**（不是放松，是它们判的那件事本身变了）：
   - ①「不丢字不重字」原来是「逐块拼 `[source_start, text_end)` == 正文去掉换行」。
     图片块的 render 文本为空，标签那几十个字符不再被画成文字 —— 现在拼的是
     「文本块取正文、图片块取整段 source range」，判的仍是「每个字都归属某个块、
     不丢不重」，而不是「每个字都被画成文字」；
   - **下线判据**（新旧两条路的页首 offset 对照）**改成在去掉图片那一行的语料上跑**。
     旧实现把 `<img …>` 当字面文本画，新实现把它当图片块 —— 拿它对页首 offset 是在比
     一件与分页无关的事。这是 M1 原话里「差异逐条有理由」的第二条理由
     （第一条仍在：旧实现末行被裁一半）。

##### M2b 欠下的账

- **图片字节没有持久缓存**（出入第 4 条）。换一章再换回来就重取。要落库先得还
  `caches` 的容量与淘汰那笔债，或者另起一个按文件存的图片目录（裁判是后者）；
- **`,{option}` 里的 `width` / `style` 不影响排版**。`format_keep_img` 会把
  `src="url,{"width":"50%"}"` 原样留着，这一侧也原样交给 `AnalyzeUrl`（它自己会切），
  但**没有**按 `width` 调整图片占多宽。要做就是在 Dart 侧解那段 JSON，那是给
  `imageBox` 多一个入参的事，不是协议改动；
- **裁判的 `ImageUtils.decode`（书源 `decodeJs` 解密图片）没做**。用到它的源极少，
  而那一步要把 JS 宿主接进图片链路。不做比留个半截实现好；
- **取图失败不重试**（**自动那条仍然不做；手动那条已开**，见 §8.6）。理由没变：
  自动重试要有退避与上限，而每次结论翻面都要重装页（代价是整章）。但**读者自己点
  那一下两样都不需要** —— 节奏由手指定，一次点击一次重取，所以取不到的图现在点一下
  会再取（`ReaderImageStore.retry`）；
- ~~**图片不可点、不能看大图**~~（**已还**，见 §8.6）。当时的判断也兑现了：它确实
  「是行级 Placement 的命中之外的一个分支，不是新协议」—— `ReaderHit` 本来就带
  `blockId`，`kindOf` 一句话就答得出「这一块是不是图片」，`Placement` 一个字段没动。

#### M2c — 滚动模式（已完成）

- [x] 滚动与分页共用 `ChapterDocument`，**页面模式独立于正文获取** ——
      换模式不换会话、不重取正文，连重新测量都不用：同一份 `measures` 装两遍
      就是两种模式（`ComposeParams::scroll`）；
- [x] 滚动下 `durChapterPos` 取**视口内第一个完整可见字**，写进契约（下面一节）。

**滚动 = 页高无穷。** 装页器一行没改：`scroll` 为真时把 `vh` 当成 `f32::INFINITY`，
于是一行都断不了，整章落成**一页**，`origin_y` 变成章内绝对坐标。「不断页」本来
就是「页高够大」的极限情形，不是另一套算法。这条让两种模式**共用同一条流水线**：
同一份块表、同一份行度量、同一个 `compose`、同一个页表线格式、同一个绘制器。

为什么参数是一个 `bool` 而不是「把 `viewport_height` 传成无穷大」：后者是个魔法值 ——
`is_finite` 那句校验得为它开口子，而那句挡的正是「Dart 那边算出了 NaN/Inf」。
模式是模式，尺寸是尺寸。滚动下 `viewport_height` 照样要合法：装页不用它，
但 Dart 拿它算图片占多高、算可见窗口。

##### 滚动下的 `durChapterPos`（写进契约）

**视口内第一个完整可见字。**「完整」= 那一行的**行顶**落在视口里；被视口上沿切掉
一半的那一行不算 —— 读者读不全它，而且下次按这个 offset 回来时它会重新落在屏幕顶上。

这条口径要能**往返**：`setScroll(scrollOffsetFor(pos))` 之后 `currentSourcePos`
必须还是 `pos`。不往返就是每换一次设置进度漂一点，几次之后就不是读者上次读到的
地方了 —— 与 §5.5「吸附只发生在消费锚点时，已落库的值不因吸附被改写」同一个理由。
判据 ⑬ 逐点验它。

换宽度之后断行会变，原来那个字可能落到某一行的中间，那时报的是**那一行的行首**
（≤ 原值）。这不是漂：它仍然是「屏幕上第一个完整可见字」，而且仍然往返。

##### 滚动怎么画

`ReaderScrollView`（`app/lib/reader/reader_surface.dart`）：一块**视口大小**的画布，
上面盖一层**空的**滚动视图。位移、惯性、越界回弹这些平台物理由 Flutter 自己的
`Scrollable` 出；画布只按当前位移画一屏。

**位移的源头是视图，不是 controller** —— 两边各滚一次就会打架。反方向只有一处：
重排/换章之后 controller 按锚点定下位移，视图在**帧后**跟上去（内容高度是那一帧
才变的，早了 `jumpTo` 会被旧的 `maxScrollExtent` 夹住）。

为什么不把画布放进滚动视图里：那样 `CustomPaint` 的画布就是整章那么高（长章几十万
像素），光栅层按整章分配 —— 而 §5.4 的结论正是「只画看得见的那几行」。
空的 `SizedBox` 没有绘制成本，它只负责把滚动范围撑出来。

滚动位置改变**不走 `notifyListeners`**：它单开一条 `ValueNotifier`（`scrollY`），
只让画布重画，不让整个阅读页重建（页号、章名、工具栏都没变）。

##### 顺带收的那笔账：视口高度不再触发整章重测

M1/M2a/M2b 里视口**高度**混在 `sizeChanged` 里一起触发 `_relayout`，而它压根不改
行度量。切一次工具栏（视口矮一条 AppBar + 底栏）就把整章重测一遍 —— 参考机上是
百毫秒量级。现在高度与模式都走 `_recompose`（M2b 为图片回流建的那条路）：
**行度量还在手里，只是重新装一次页**。实测见 §10 的 M2c 那张表。

验收（M2c）：

- [x] **七条判据在 macOS 与 Android 参考机各过一遍**
      （`app/integration_test/reader_layout_m2_test.dart` 的 ⑫～⑱）。参考机上
      **连跑两遍**；
- [x] **窗口连续缩放**（原话里的那一条）—— 判据 ⑯：七组宽高各自零碎地变、
      每组再分页/滚动各来一遍，每一步都验分页不变量 + 诊断为 0 + 读者还看得见
      原来那段正文；
- [x] **旧 PagePlan 不会覆盖新设置**（原话里的那一条）—— 判据 ⑰ 连着切五次模式；
      机制见「出入」第 3 条；
- [x] M1 的八条、M2a 的七条、M2b 的四条都不回归（两台各一遍）；
      产品路径 `app_test.dart` 两台不回归；
- [x] Rust 侧新增用例：`reader-layout` 6 条（整章一页且 `origin_y` 绝对 / 两种模式
      共用同一份行度量 / 滚动的前缀装页只往下长 / 滚动下没有超高行也没有并页 /
      空章 / 滚动也拒绝退化视口），随机用例里每一轮再顺带跑一遍滚动；`engine` 1 条；
- [x] Dart 侧新增用例：设置 2 条、面板 1 条。

##### M2c 与计划的出入（三条，各带理由）

1. **滚动模式没有单开一条装页路径**，是 `compose` 的一个参数。计划书原话是
   「滚动与分页共用 `ChapterDocument`」—— 实际共用的比那多得多：块表、行度量、
   `compose`、页表线格式、绘制器全是同一套，滚动只是「页高无穷」。
   这条一路省到底：线格式没升版本、Dart 侧没有第二个页表解码器；
2. **锚点的正反两个方向都在 Dart**（`currentSourcePos` / `scrollOffsetFor`），
   不在 Rust。§1 说「阅读锚点在 Rust」，但滚动的锚点要的是**行度量**（某一行的
   `top`），而行度量按 §4.1 就在 Dart —— 让 Rust 算它就得把行度量再留一份。
   分页那条的 `page_for_source` 两侧本来也都有，消费方用的是 Dart 那份；
3. **「旧 PagePlan 不覆盖新设置」不能只靠代号**。模式与视口高度**不进代号**
   （它们不重新测量，走的是重装页），所以代号挡不住「用旧模式装出来的页表落到
   新模式上」。`_compose` 因此把**装页用的那套设置记下来**，落地前再对一次：
   对不上就整份丢掉。这不是理论风险，见下面那两个 bug。

##### 这一片抓到的两个真 bug（都要靠参考机才现形一个）

1. **换模式时读者被扔回章首。** `updateLayout` 先把 `_scroll` 改了、再读
   `currentSourcePos` —— 那是拿新模式的解法去解旧页表：滚动那条分支读的是
   `scrollY`（还是 0）与分页的页表，结果恒为 0。修法是**先按旧模式读出锚点**
   再改状态。macOS 上也现形；
2. **在飞的那次装页把锚点擦了。** `_compose` 发布时无条件 `_pendingAnchor = null`，
   而那时 `_pendingAnchor` 可能已经被后来的换模式重新记上了。**参考机上必现**
   （取图失败触发的重装页正好落在换模式的前一刻），macOS 上因为够快而看不见。
   修法两条：**只有读了它的那一次才能清它**，以及上面出入第 3 条的设置对账。

还有一个是 benchmark 抓的，不是判据抓的：**「锚点还没兑现」不该一律走整章重排**。
M1 里只有重排这一条发布路径，所以 `anchorPending` 当年直接走 `_relayout`；现在重装页
也能发布，而「还没量过一个字」（`_pages == null`）才是非重排不可的那一种。分不开的
代价是实打实的：换一次模式先被挡下（那时有别的装页在飞），紧接着的那次
`updateLayout` 就把整章重测一遍 —— 参考机 200k 那档 **583 ms**，修完 **8 ms**。
判据全绿的时候它还在，是那张表把它照出来的。

##### M2c 欠下的账

- **滚动下不能连着章读**。滚到底要点一下才进下一章，不是像裁判那样把下一章接着
  往下排。接着排要让 `ChapterDocument` 跨章拼，那是 `durChapterPos` 的口径问题
  （它是**章内**下标），不是一个滚动参数；
- **滚动下没有「记住上次滚到哪个像素」**。落库的是 `durChapterPos`（第一个完整
  可见字），回来时那个字落在屏幕顶上 —— 与上次的像素位置差半行以内。这是有意的：
  像素位置换个字号就没有意义；
- **`scrollProgress` 按像素算，不按字数**。长图会让百分比走得比正文快；按字数算
  又会在图片多的章里显得卡住。两种都不完美，先用像素（它至少与滚动条一致）；
- **M2a 那笔「字号重排到首屏压线」仍未还**，而且这一片给出了「按原方案做反而更差」
  的理由 —— 见 §10 的分析。

### M3 — 中文排版能力

任务：

- [x] 建中文排版规则用例集：每条规则一组语料 + 人写的期望断行/分页结果 —— **M3a**；
- [x] 行首/行尾禁则 —— **M3a**（实测由 ICU 免费给了，落地的是判据，见 §7.2）；
- [x] 标题不可拆、孤行控制 —— **M3b**（孤行控制落地；标题不可拆没有落地对象，
      理由见那一片的出入第 1 条）；
- [x] 两端对齐 —— **M3c**（顺带把 M2a 欠的「标题居中」还了）；
- [x] 页底对齐 —— **M3d**；
- [x] 标点悬挂、标点压缩 —— **M3e 的 spike 做完了，结论是这一版不做**，
      门并到 M5；四条实测证据见那一片；
- [x] 从 `judge/engine` 抄禁则字符表与压缩规则的语料，逐条标注来源行号 —— **M3a**
      抄了禁则那两张（压缩表留给 M3d，它的消费者在那一片）；
- [x] 中文/英文/数字/emoji/RTL 混排回归 —— **M3a**（抓到一个真 bug，见下）。

验收：每条规则的用例集 0 FAIL；三平台各过一遍；不设与上游的页码对照。

#### M3a — 禁则与规则用例集（已完成）

- [x] 规则用例集 `app/test/reader_layout_zh_rules.dart`：禁则语料 20 条 + 混排语料
      8 条，八条探针，三平台（flutter_tester / macOS / 参考机 HITV205N）各过一遍
      （`test/reader_layout_zh_rules_test.dart` 与
      `integration_test/reader_layout_m3_device_test.dart`）；
- [x] 禁则字符表抄自裁判并逐条标注来源行号（`ZhLayout.kt:27-30` `postPanc`、
      `ZhLayout.kt:31` `prePanc`，并上 `PunctuationCompress.kt:32-33` 的
      `openChars`/`closeChars`），**去掉 `<` `>`**，理由见 §7.2；
- [x] 「无处可断」豁免 + 判据不恒真的反向用例；
- [x] 混排回归：中/英/数/emoji/emoji 连字/组合符/RTL/全角空白。

##### M3a 与计划的出入（三条，各带理由）

1. **禁则没有落成「Rust 纯函数规则模块」**（§2.1 目标 6 的原话）。落地对象不存在：
   断行的是 ICU，Rust 这一侧一个字符也不消费这张表。此刻在 Rust 建一个没有调用方的
   表，正是 `reader_doc` 那段注释点名的反面教材（「提前造一个恒等的映射结构只会让人
   以为它已经在做事」）。M3d 真让 Rust 断行时表移过去，**同一份语料**当两侧共用的判据；
2. **判据不是「逐字对人写的期望断行」**（M3 任务第 1 条的原话）。人写的期望只在
   全角等宽的语料上稳——参考机上 `“` 的 advance 只有 15.4（§7.2 的 M0 实测），
   按「每行 N 字」写死的期望一到真机就全错。所以主判据是**规则检查**：对任意一份行区间
   判「这个断点合不合规」，与字体无关。人写期望只留在「无处可断」那两条上，
   因为那两条的正确答案与宽度无关；
3. **判据走产品那一条取行区间**（`ParagraphBackend.lineBoxesOf`，为此把它从私有
   提成 `@visibleForTesting` 的静态）。判据另写一份取行区间的代码，就会出现
   「判据全绿而正文是坏的」——那正是 Phase 3 记下的「裁判侧是桩」那个形状。
   真身由**第三条独立 API**（`getLineBoundary`，慢到不能进产品但不受 bidi 影响）
   逐条对照，见探针 ⑥。

##### 这一片抓到的真 bug：RTL 段落的行区间整份偏移

「中文/英文/数字/emoji/RTL 混排回归」这条任务写在计划书里两个月，抓到的东西比预期大：

`ParagraphBackend._lineBoxes` 从 M1 起用 `getPositionForOffset(Offset(0, 行顶))` 取行首。
它取的是**视觉左缘**那一点落在哪个字符上——纯 LTR 下恰好等于行首，但一行里混进 RTL 时，
RTL 段的视觉左缘是它的**逻辑末尾**。一段阿拉伯语正文因此整份行区间都往后偏
（实测 `[5, 13, 21, 30, …]`，真身是 `[0, 6, 14, 22, …]`）：`lineText` 切错正文，
行区间倒挂时直接 `RangeError`。

修法是换第三条路（`getLineNumberAt` 的指数外推 + 二分，§5.3 有完整说明）。
**换完顺手把 §10 那笔「字号重排压线」的账还了**——那笔账从 M1 记到 M2c，
M2a 设想的解法（后缀装页）在 M2c 被论证为「做出来更差」而搁置；真因其实有一半在这里：
`getPositionForOffset` 每行的成本随**行数**线性，长章重排时它是实打实的一截。
参考机 50k 那档 162 ms → **122 ms**（门槛 150 ms），200k 整章补齐 718 → **565 ms**。

这条也给「判据会骗人」添一个形状：**判据全绿，只是因为语料里没有那种字**。
M1/M2 的语料全是中文，八套判据一条都没碰过 bidi；把 RTL 写进语料是计划书两个月前
就列着的任务，做了才现形。

##### M3a 欠下的账

- **压缩表（`PunctuationCompress.kt:32-33` 的 `openChars`/`closeChars`）只当了禁则表的
  并集来源，压缩规则本身没抄**。它的消费者在 M3d（标点挤压要 per-glyph 墨水盒），
  在那之前抄进来就是一张没人读的表；
- **`<` `>` 仍会出现在行首/行尾**。理由见 §7.2，这是**选择**不是遗漏；真要它们，
  代价是整条 §7.2 第二阶段；
- **混排判的是「行区间这个结构没坏」，不是「断得好看」**。RTL 正文在中文竖排/悬挂
  规则下该怎么排，不是这一版要回答的问题；
- **没有 golden**。M3a 一个像素也没改（换的是取行区间的路，同一份 LTR 语料逐位相同，
  M1/M2 两套真机判据全绿即证）。golden 等 M3c 的两端对齐进来再建——那时它才开始
  能挡住东西。

#### M3b — 孤行控制（已完成）

- [x] 孤行控制：一个块被拆到两页上时，**页底与次页顶各至少 2 行**
      （`reader_layout::compose::plan_split`，`DEFAULT_MIN_SPLIT_LINES = 2`）；
- [x] 用例：两行段落不拆、页尾孤行往回拉、页首孤行整段下推、续排块不受页首那条约束、
      页还空着时下推没有去处、滚动下是空操作、不动图片块、前缀装页仍与全量逐页相同、
      随机输入下规则真的兑现；
- [x] 真机判据（`integration_test/reader_layout_m3_device_test.dart` 的 ⑨⑩⑪）：
      五种视口、23 个被拆开的块，页底与次页顶都 ≥ 2 行；分页不变量不破；
      窄到 80 px 的视口也没有排出空页。

**规则本身**：本页从第 `i` 行起、能装到第 `j` 行时——

- **页尾孤行**（widow）：切完剩下的行少于 2，把切点往回拉到 `len - 2`；
- **页首孤行**（orphan）：这一块**刚在本页开头**（`i == 0`）却只留得下不到 2 行，
  整块下推到下一页。

两条一起的效果：**行数 ≤ 3 的段落永远不拆**，4 行的段落只能拆成 2 + 2。

##### M3b 与计划的出入（三条，各带理由）

1. **「标题不可拆」没有落地对象**，所以没做。写到一半才发现：Rubato 的标题
   **恒为第 0 块**（`ChapterDocument::build_with_title`，切块那一遍只把第一行记成标题），
   而第 0 块永远落在第 0 页的页顶——「整块下推」在页还空着时没有去处，
   「与下一块同页」在页还空着时也无从判起。两个标志位（`keep_together` /
   `keep_with_next`）连线格式 v4 都写完了，又整个删掉：它们对唯一的标题块**恒为空操作**。

   这与 M2a「不为居中先开一个位」是同一条。真要它们时的落地对象是清楚的——
   正文里出现第二级标题（`<h2>` 之类）的那天，那时是切块那一遍多认一种块，
   加位是顺带的事；

2. **线格式一位没动，`compose_pages` 的签名也没变**。`min_split_lines` 是
   `ComposeParams` 上的一个数，由引擎侧按 `DEFAULT_MIN_SPLIT_LINES` 填死。
   它是**排版惯例不是设置**：开成滑块就得回答「1 是什么意思」，而 1 的意思是
   「关掉这条规则」。用例可以传 1，那是为了让「这条规则到底改了什么」有个对照
   （M1/M2 那些逐页对照的用例继续跑在旧口径上——换成新口径会让「这条用例在判什么」变模糊）；

3. **两条规则不对称**：页首那条只在 `i == 0` 时管，块是从上一页续排来的（`i > 0`）时
   只要能推进一行就行。少了这条不对称，长段落在窄页上会一路把自己往后推——
   那几行不是段首，拿段首的标准要求它们没有道理。

##### 这一片的两个坑（都是用例先撞上的）

1. **「整块下推」必须看「页还空着吗」**。不看的话，块比页高时它会把自己推回同一个
   空页，推到天荒地老。用例 `页还空着时下推没有去处` 钉的就是这条：视口只装一行时
   仍然是「每行一页」，而且**不记诊断**——做不到不是异常，诊断位留给真正的异常；
2. **随机用例的「规则兑现了吗」第一版是错的**，它把两档**做不到**当成了违规：
   一页装不下 2 行、或整块的行数不到 4（3 行的段落落在只装 2 行的页上，
   2+1 与 1+2 都违规）。这两档在 Rust 的随机用例与 Dart 的真机判据里**各写了一遍**，
   是一处会漂的地方，记在账上。

##### M3b 欠下的账

- **标题 + 一行正文的页会被孤行控制排空成「只有标题」**。视口恰好只装得下
  标题加一行正文时，正文那一块被下推，标题独占一页——比「标题挂在页底」更难看。
  挡它要么是 `keep_with_next`（对块 0 恒为空操作，见出入第 1 条），要么是一条新规则
  「一页至少要有几行」。极窄视口才撞得上，先记着；
- **「做不到」的口径写在两处**（Rust 的随机用例、Dart 的真机判据），两边各自漂就是
  「判据全绿而规则没兑现」。真要收，得让 `compose` 把「这一页是做不到的那一种」
  报出来——那是 `ComposeDiagnostics` 上的一个新位，但它不是异常，混进诊断里会让
  「非零即异常」这条口径失效；
- **页底空白比以前多**。孤行控制把行往下赶，页底会空出一到两行——**页底对齐**
  （§7.2 第 7 项）正是收这个的，在 M3c；
- **`min` 不可调**。见出入第 2 条。

#### M3c — 两端对齐与标题居中（已完成）

- [x] 两端对齐（`ReaderPaintStyle.justify`，默认**开**，设置面板可关）；
- [x] 标题居中 —— M2a 记在账上的那条；
- [x] 判据：逐字位置与整段 justify **逐位相同**、末行与空行不拉伸、
      不改断行也不改行高、换它只重绘不重新测量；真机三条（⑫⑬⑭）。

##### 关键的一手：按行画时 `justify` 要挂一个占位符

`TextAlign.justify` **从不拉伸末行**（排版惯例），而按行画时每一行都是它自己那份
Paragraph 的唯一一行——也就是末行，于是 justify 一点效果都没有。§7.2 的 M0 实测
（「用整段 Paragraph 就够」）默认的是整段绘制，而绘制早在 M0 就被真机推翻成按行了，
这一处衔接**当时没人对上**。四条路都实测过（视口 207，10 个汉字 + 7 px 余量）：

| 走法 | 行右缘 | 结论 |
|---|---|---|
| 整段 `justify`（参照） | 207.0 | 逐字左缘 0 / 61.556 / 103.111 / 144.667 / 186.222 |
| 单行 `justify` | 200.0 | **不拉伸**——它是末行 |
| 单行 + `\n` / 零宽空格 | 200.0 | 仍然不拉伸：硬断出来的那行照样算末行 |
| 单行 + `letterSpacing` 手摊 | 208.2 | 摊得出宽度，但字距加在**每个字后面**（首字左缘缩进半格），而且拉丁文行该拉的是词距不是字距 |
| **单行 + 尾随占位符** | 207.0 | 逐字左缘与整段**逐位相同** |

占位符（`ParagraphBuilder.addPlaceholder`，宽度取整个视口宽）把这一行变成
「不是末行」，justify 于是照常工作。它是独立的 run，**不参与整形**——哨兵字会改前一个
字的连写形（阿拉伯语），占位符不会；`drawParagraph` 也不画它。

##### M3c 与计划的出入（三条，各带理由）

1. **两端对齐是绘制键，不是几何键**。§7.2 的实测已经把前提摆在那儿了：justify 对
   纯 CJK 行只均摊字距，**断点与行高都不变**。所以它进 `ReaderPaintStyle`——换它
   只重建行 Paragraph，不重新测量、不重新分页（判据 ⑫ 与 M2a 的「换配色只重绘」
   同一条口径）。用例里另有一条**判那个前提本身**：一旦 justify 改了断行或行高，
   它就不再是绘制键了，那条会先红；

2. **`LineMeasure` 那个 `width` 位没加**。M2a 的账上写着「M3 的两端对齐真要 `width`
   时一起加，不为居中先开一个位」——结果**两端对齐和居中都不要它**：对齐由行
   Paragraph 自己按视口宽做（justify / center 都是 `TextAlign` 的事），装页器一位没动，
   页表也没动。这是「不为将来先开位」第三次被兑现（前两次是 M2a 的标题、M2b 的图片）；

3. **标题居中没做成设置**。它不是口味，是标题的排法；开成开关就得回答
   「左对齐的标题算什么」。两端对齐**是**设置（有人不喜欢字距被拉开），默认开。

##### 这一片撞上的判据坑：`complete` 不等于「不会再来一份」

`reader_layout_m2_test.dart` 的④「换配色只重绘、页表逐位不变」在参考机上开始**偶发**红
（三次里红一次），macOS 上从不红。真因不在 M3c：随包语料自带一张 `<img>`，它取不到时
会在 120 ms 之后触发一次**重装页**，而那一次会换掉页表对象。快照拍在它之前，
「换配色换掉了页表」就成了它的锅。M3c 只是把时序挪了一点（建带占位符的行 Paragraph
慢一点），把这个一直在的坑挪进了窗口。

修法是给 controller 加一个 `layoutSettled`（没有在飞的、也没有欠着的重排/重装页），
判据等的是 **`quiet(c)`**：整章测完 + 排版落停 + 每张图都有结论。
`pageTable.complete` 只说「这一份是整章的」，**不说「不会再来一份」**——这是
M2b/M2c 那两个「等状态变了要等最后一步」的第三种形状。

##### M3c 欠下的账

- **翻页那一帧贵了 0.4 ms**（参考机 1.72 → 2.1 ms/页，帧外补邻页 23 → 27 ms）。
  建带占位符的行 Paragraph 比裸行贵一点。门槛是 build+raster p95 ≤ 8 ms，仍在预算里，
  但这是 M1 以来第一次翻页成本**涨**；
- ~~**拉丁文行拉的是词距还是字距，没验**~~（**验了**，见 §8.6）。M3 真机那套加了
  第 ⑰ 条：一行西文里 **7 处词距被拉开、24 个字在词内一动没动** —— 拉的是词距，
  这是对的。判据必须判**字的位置**：两种拉法行宽一模一样，判行宽会同时全绿；
- **页底对齐没做**（M3d）。孤行控制把行往下赶，页底空白比以前多，收它的正是那一片。

#### M3d — 页底对齐（已完成）

- [x] 页底对齐：把「差一点点就满」的那截空白均匀摊进行距，让每一页的正文下缘对齐；
- [x] 页表的摆放记录多一个 `line_gap`（占的是 v1 那个 `reserved` 位，**记录长度没变**，
      版本升到 4）；绘制方按 `y = origin_y + line.top + (i - line_start) * line_gap` 画；
- [x] 用例九条 + 真机两条（⑮⑯）。

**规则**：一页排完之后 `leftover = 页高 − 末行行底`，摊到 `n − 1` 道行距上。
撑多少算多，看的是**每一道行距被撑开多少**，不是「这一页空了多少」——同样空一行，
二十行的页摊下去每道 5%（看不出来），三行的页摊下去每道 50%（当场露馅）。
上限 `MAX_BOTTOM_GAP_RATIO = 1/8`（32 px 的行最多加 4 px）；超了**整页不撑**，
不摊一半（摊一半照样不贴底，只是行距还变了）。**末页不撑**：它本来就短，
而且分批测量时还会往下长。滚动模式没有「页底」这回事。

实测（随包语料拼到两万字，`plain.txt`）：360×640 的视口下 **52 页全部贴底**；
300×400 是 92/99；420×300（一页只有六七行）是 70/103——页越矮，每道行距要撑的越多，
被上限挡下的也越多。这正是这条上限想要的样子。

##### M3d 与计划的出入（两条，各带理由）

1. **`line_gap` 在「摆放」上，不在行度量上**。同一份行度量在不同的页上摊到的数不一样
   （每页剩多少空白不同），所以它是「这一页怎么摆」的一部分，不是「这一行有多高」。
   放进行度量就等于让同一行在换页之后必须重测；

2. **占的是 v1 那个 `reserved` 位，记录长度没变**（32 字节）。位没变、语义变了，
   所以照样升版本（M2a 的 `space_before` 是同一手）。三段缓冲共用一个号，
   块表与行度量这次一位没动。

##### 这一片是看着 PNG 定的，不是算着定的

第一版的口径是「`leftover` 比本页平均行高还大就不摊」。它在 360×640 下把 52 页里的
12 页判成「本来就短」——而那 12 页多半是孤行控制推下去一块留下的，摊到二十道行距上
每道才 1.7 px，肉眼根本看不出来。**判据换成「每一道行距撑多少」之后，同一台机器上
52 页全部贴底**。

改口径的依据是画出来的 PNG（`Directory.systemTemp` 里存三页，页底画一条红线），
不是哪个公式更好看：两端对齐、标题居中、页底对齐都是**看的**东西，
算得对不代表看着对。

##### M3d 欠下的账

- **两端对齐在「短行」上会拉得很开**。一行只装得下十来个字、又被禁则往回拉了一两个字时，
  余量摊到十几道字距上就是肉眼可见的稀疏（PNG 里那句「他 没 有 回 头 ， 只 把 那 卷
  《 九 章 》 塞 进 袖」）。收它的正是 **M3e 的标点压缩与悬挂**——把标点占的那半格
  挤出来/挂到版心外，行里就多出容纳一两个字的宽度。这是这一版看得见的最后一处；
- **`MAX_BOTTOM_GAP_RATIO = 1/8` 是看出来的，不是算出来的**。它挡住的是「三行的页
  撑 50%」那种，放过的是「二十行的页撑 5%」那种，中间那一段（六七行的页撑 15%）
  两边都不像。真要调，得有更多语料与更多屏幕；
- **没有 golden**。理由与 M2b 那条一样：真要挡住的是「摊完不许超出页底、行序不许乱」，
  那是**算得出来**的不变量（判据 ⑮⑯ 判的就是它），不是像素比对。

#### M3e — 标点悬挂与压缩：spike 做完，**这一版不做**

§7.2 最后一段要求「进入第二阶段前先做独立 spike……不能直接进入主实现」。
spike 在 `app/test/reader_layout_m3e_spike.dart`（host / macOS / 参考机三处跑），
**只报数不设门**。四条实测，结论是这两项在当前架构下要么不划算、要么做不到。

##### ① 逐字宽度：逐位置取是 O(n²)，只能按**不同字符**去重

Flutter 没有裁判 `ZhLayout` 用的那种「一次拿一整行 advance」的 API
（Android 的 `Paint.getTextWidths`）。逐位置问的两条路都随本 Paragraph 的长度线性增长：

| 字数 | `getBoxesForRange` 逐字 | `getGlyphInfoAt` 逐字 | 只测不同字（78 个） |
|---|---|---|---|
| 500 | 40.9 ms | 45.0 ms | 22.8 ms |
| 5 000 | 4 573 ms | 4 636 ms | 5.2 ms |
| 20 000 | **92 803 ms** | 93 154 ms | 5.0 ms |

（参考机，profile。macOS 上分别是 3.6 / 298 / 4 810 ms。）
20 000 字要**一分半**，这条路直接封死。

去重之后单价是「一个字建一份 Paragraph」：参考机 **338–365 µs/字**（冷），
macOS 33–36 µs/字。一章中文的不同字数是四位数量级，也就是**参考机上
0.37 s（1 000 字）到 1.0 s（3 000 字）**——一次性、按 (字体, 字号) 缓存，
可以在后台摊，但它是实打实的一笔。

##### ② 一张「字符 → 宽度」表对中文精确，对**连写脚本是错的**

逐字宽度加起来等于整行宽度吗（`maxIntrinsicWidth`，参考机 / macOS）：

| 语料 | 参考机 Δ | macOS Δ | |
|---|---|---|---|
| 纯中文 | 0.00 | 0.00 | 一致 |
| 中文标点 | 0.00 | 0.00 | 一致 |
| 中英混排 | 0.58 | 1.27 | 差一点（kerning） |
| 拉丁文 | 0.45 | 2.59 | 差一点 |
| **阿拉伯语** | **37.26 / 88.47** | **27.20 / 88.36** | **差三到四成**（连写） |
| 组合符 / emoji 连字 | 0.00 | 0.00 | 一致（按 grapheme cluster 量） |

所以「Rust 拿一张字符宽度表自己断行」这条路**只对 CJK 成立**：含连写脚本的块必须
退回让 ICU 断。那意味着两种断行来源并存——协议撑得住（`Placement` 从第一版就是
行级的），但行表的方向要翻过来（现在是 Dart → Rust，那时对 CJK 块变成 Rust → Dart），
而且一次重排要多一张字符宽度表 + 多一次往返。

**顺带纠正一个量法**：第一版用 `LineMetrics.width` 量单字，它把行尾空白剪掉，
逐字量一个空格得 0，于是「和」凭空少掉每个空格的宽度——第一版就是这么报出
「拉丁文不一致」的。换 `maxIntrinsicWidth` 之后才是真的。**量法错了，结论会反**。

##### ③ 按**任意**切点重建 Paragraph，字形一位不动

Rust 自己断行会切在 ICU 不会切的地方（悬挂把标点挤到行尾、压缩把两个标点并成一格），
所以「切出来重建 == 整段里那一段」这条对**任意**切点都要成立，不只对 ICU 的切点。
实测（把一句中文在每一个位置切开重建，逐字左缘对照整段）：
**参考机与 macOS 都是 0.0000 px**。这一条是好消息：绘制侧不拦路。

##### ④ 标点压缩要**逐字画**，那是 5.2–5.8 倍

压缩改的是行内逐字 x，而一行只有一份 Paragraph 时给不出逐字 x——只能一个字一份
Paragraph 地画。一页 19 行 / 388 字：

| | 按行画（现在） | 逐字画 | 倍数 |
|---|---|---|---|
| 参考机 录制 | 0.837 ms | **4.834 ms** | 5.8× |
| 参考机 含光栅 | 12.8 ms | 38.5 ms | |
| macOS 录制 | 0.036 ms | 0.187 ms | 5.2× |

§10 给一页绘制的门槛是**录制 p95 ≤ 2 ms**。4.83 ms 是它的 2.4 倍——
**标点压缩在当前绘制模型下过不去这道门。**

##### ⑤ 悬挂/压缩真正能挤出多少（真字体，字号 40）

| 字 | advance | 墨左 | 墨右 | 可挤掉 |
|---|---|---|---|---|
| `，` | 40.0 | 5.0 | 25.0 | 30.0 |
| `。` `、` | 40.0 | 1.0 | 26.0 | 27.0 |
| `：` `！` | 40.0 | 6.0 | 26.0 | 32.0 |
| `（` `「` | 40.0 | 25–27 | 1.0 | 26–28 |
| `）` `」` | 40.0 | 1.0 | 25–27 | 26–28 |
| `“` `”` | **15.4**（macOS 18.4） | 2.0 | 0.4–2.4 | 2.4 |
| `中`（对照） | 40.0 | 3.0 | 3.0 | 6.0 |

一个句号能挤出 **0.65 em 以上**——这正是两端对齐在短行上拉得很开的解药（M3d 的账）。
顺带第三次证实「CJK 标点不一定是 1 em」：`“` 在参考机上 15.4、macOS 上 18.4，
**宽度一律实测**。

（M0 的探针④那张表里 `中` 报的是「没有墨」：它用默认颜色（黑）把字画在黑底上。
这一版把字压成白的，扫的才是真的墨迹。）

##### 结论与决定

- **标点压缩：不做。** 它要逐字画，而逐字画是 §10 一页绘制门槛的 2.4 倍（④）。
  要它得先换绘制模型——那正是 §8 **M5「原生 Rust 渲染决策门」**要判的事，
  门并到那里，不在 M3 里开半扇；
- **标点悬挂：不做，但理由不同。** 它不要逐字画（行照常一份 Paragraph，
  只是允许它的墨迹越出版心一点），**卡的是断行**：要「行尾挂出去所以多装一个字」，
  断行就得由 Rust 拿字符宽度表来做，而那要付①的一次性 0.37–1.0 s、②的
  「连写脚本退回 ICU」两套断行来源、以及一次重排多一张表加一次往返。
  为一个标点的宽度付这些，在**悬挂单独一项**上不划算；它和压缩是同一套地基，
  一起放到 M5 那道门后面判；
- **③ 是这次 spike 唯一的好消息，而且要记住**：绘制侧对任意切点是安全的
  （0.0000 px）。M5 真要走「Rust 排字」，这一条已经验过了，不用再验。

##### M3e 欠下的账

- **两端对齐在短行上仍然会拉得很开**（M3d 的 PNG 里那句）。收它的解药已经量出来了
  （⑤：一个句号 0.65 em），但取解药的代价在 M5 那道门后面；
- **spike 的语料是一段话重复出来的**（只有 78 个不同字）。①里「一章有多少不同字」
  是拿合成的连续汉字量的单价推的，不是真书统计。真要锁那笔一次性开销，
  得先有真语料的字频；
- ~~**没量「字符宽度表按 (字体, 字号) 缓存」的命中率**~~（**量了，而且问题当场消失**，
  见 §8.6）。spike 加了第 ⑥ 条：把宽度归一到「每 em 多宽」之后，跨 9 档字号
  （含 17.3 / 21.7 这种 textScaler 摊出来的非整数）的**最大相对偏差**——
  **macOS 0.000%，参考机 0.058%**（最差那个是「i」@ 21.7）。
  也就是说**键里根本不用放字号**，表存「每 em 多宽」，滑块滑到哪儿都命中 ——
  ① 量出来的那笔一次性开销（参考机 0.37–1.0 s）**一辈子只付一次**，不是每滑一下付一次。
  这对 M5 的算式是实打实的一格：悬挂那条路的成本比 M3e 收盘时以为的低一个数量级。

  **参考机那个 0.058% 不是零，值得看清它是什么。** 最差的是「i」——**最窄**的那个字形，
  而且在一个非整数字号上。相对误差在最窄的字形上最大，正是「绝对舍入量恒定」的样子
  （字体引擎按定点数存 advance），不是「误差随字号线性放大」。所以它**不累积成比例漂移**：
  360 px 的一行摊到头也就 `360 × 0.058% ≈ 0.21 px`，在一个像素以内。
  真让 Rust 拿这张表自己断行时，这一档误差只可能让**恰好卡在行末边界上**的那一个字
  换一行，不会把版面算坏 —— 但那意味着 Rust 断出来的行与 Flutter 自己断的**可能差一个字**，
  这条要和 ② 那条（连写脚本上「和 ≠ 整体」）一起进 M5 的判子。

### M4 — 选择、批注与阅读交互基础

**分片**（与 M2/M3 同样的拆法，一片一次收盘）：M4a 命中测试 / M4b 选择与复制 /
M4c 高亮与下划线（含落库与重排后重绑）/ M4d 搜索命中 / M4e 翻页动画。

任务：

- [x] 点击坐标 → Paragraph offset → SourceMap offset（命中测试建立在 §5.4 的行级
      Placement 上，与 M3 换行来源无关）—— **M4a 落地**；
- [x] 跨行/跨页选择、复制 —— **M4b 落地**；
- [x] 高亮、下划线与点击区域 —— **M4c 落地**；
- [x] 搜索命中 —— **M4d 落地**（章内）；
- [x] 选择和高亮在重排后仍绑定 source range —— **M4a/M4b/M4c 各有判据**；
- [x] 为未来朗读同步保留 source range 接口，但不在本计划实现 TTS 服务 ——
      `visibleSourceRange`（M4a）与 `textInSource`（M4b）就是那个口子；
- [x] 翻页动画只消费相邻 PagePlan，不参与分页计算 —— **M4e 落地**。

验收：emoji、组合字符、跨段选择不拆坏 offset；重排后批注位置稳定。

#### M4a — 命中测试：坐标 ↔ source 下标

**落地**（2026-09-02）。实现在 `app/lib/reader/reader_hit_test.dart`（一个挂在
`ReaderLayoutController` 上的 extension，全部走它的公开面），判据是
`app/integration_test/reader_layout_m4_test.dart` 的十四条，macOS 与参考机
（HITV205N）各跑一遍全绿。

两个方向都做了，因为它们是同一件事：

- 正向 `hitAtLocal(Offset)`：屏幕上一点 → `ReaderHit`（source 下标 + 落在哪一块的
  第几行 + 那一行的格子 + 点是不是真落在行身上）；
- 反向 `rectsForSource(start, end)`：一段 source 区间 → 这一刻屏幕上的几块矩形。
  **选择、高亮、下划线、搜索命中是同一件事的四种画法**，几何只算这一次；
- 外加 `visibleSourceRange`：这一刻真画出来的正文区间（**不等于**页表那一页的
  `[sourceStart, sourceEnd)`——后者含块间换行与还没测到的尾巴）。朗读同步要的是前者，
  §8 那条「为 TTS 保留 source range 接口」就落在它上面。

##### 出入与理由

1. **绘制、备行、读屏标签、命中收敛到同一条遍历**
   （`ReaderLayoutController.forEachVisibleLine`，画布局部坐标）。在这之前分页与滚动
   各有一条：绘制器一个 `if`、读屏标签一个 `if`、备行又一个。命中是第四个消费方，
   而它最经不起漂——漂了就是「点中的不是画出来的那个字」，看着像手指不准。
   `ReaderPainter` 的两个分支因此**整个删掉**了，`_scrollBody` 那半也没了；

2. **§5.4 与 `paragraph_backend.dart` 里「M4 的命中测试要整段 Paragraph 时按需重建」
   这句话是错的，已划掉**。整段那份是按 `TextAlign.start`、不摊行距、标题不居中量的
   （它的用处只是取行度量），而屏幕上那一行是 M2a 的标题居中、M3c 的两端对齐、
   M3d 摊过行距之后的样子。拿它反算坐标，标题那一行差半屏。判据 ② 把这条钉住了：
   同一个点交给整段 Paragraph 与交给画出来那一行，**必须给出不同的字**——
   哪天它们一样了，说明有人把绘制改回去了；

3. **`getPositionForOffset` 回的是插入点，不是「点中了哪个字」**。它给的是离这一点
   最近的那道**光标缝**：点在一个字的左半边回它前面那道缝，右半边回后面那道。
   照着它写，右半屏整片都偏一个字。所以取到插入点之后往回看一格，用**画出来那一行**
   的盒子判「上一个字是不是真盖住了这一点」。盒子来自那一行自己，所以两端对齐摊过的
   字距、bidi 重排过的视觉顺序都已经含在里面；

4. **零高的行不参与命中**。空块排出来是一条零高的行（`measure('')` 回
   `LineBox(0,0,0,0)`），它在屏幕上不占面积，手指压不到一条没有厚度的线上；可它到
   那一条 y 的「距离」恰好是 0，会把上下两行都比下去。**而页底对齐让分页与滚动下
   那条 y 差几像素**，于是同一个点在两种模式里吸到不同的邻居——判据 ⑦ 在参考机上
   正是这么红的（macOS 上绿，两边的 `lineGap` 不一样）；

5. **吸附按 §5.5 向前吸到 grapheme cluster 边界，且只在消费时吸**。`snapToCluster`
   是一条纯函数。`getPositionForOffset` 一般已经回的是边界，但「一般」不是契约——
   emoji 的 ZWJ 序列、变体选择符、组合记号各有各的脾气，而下标一旦切在 cluster 中间，
   `substring` 出来就是一个孤儿代理对：复制出去是乱码，存进批注就永远画不回原处。

##### M4a 顺手翻出来的两处真错

**其一：两端对齐把段首缩进整个吃掉了**（M3c 上线时就有，一直没人看见）。

段首那两个全角空格是正文里的真字符（§5.1），而 `TextAlign.justify` 按 Unicode 的规矩
把**行首空白压成零宽**，再把省下的宽度摊给后面的字。实测（360 宽、18 号、macOS）：

| | 首字 `　` | 次字 `　` | 第三个字 | 画出来第一行的墨从哪起 |
|---|---|---|---|---|
| `TextAlign.start` | 0.0–18.0 | 18.0–36.0 | 36.0–54.0 | x=36 |
| `TextAlign.justify` | **0.0–0.0** | **0.0–0.0** | 0.0–20.7 | **x=3（顶格）** |

**不是 M3c 那个行尾占位符花招惹的**：整段 `TextAlign.justify` 的多行 Paragraph 一样
吃掉缩进（同一组数）。它是 Skia 的两端对齐本身。

解法是把前导缩进**换成一个等宽的占位符**——占位符不是空白，justify 既不压它也不摊它
（实测 `0.0–36.0` 原样保住，后面的字 18.7，该摊的照摊）。代价是行内下标与 Paragraph
内下标错开 `folded - 1` 位，由 `paragraphIndexOf` / `lineIndexOf` 这一对纯函数换算；
折不折由 `ParagraphBackend.foldedIndent` 这条**纯函数**定——绘制与命中各按自己手里的
参数算一遍，不共享一张表（那种表迟早与真画出来的那份漂开）。缩进有多宽只能用
`maxIntrinsicWidth` 量：缩进是空白，`LineMetrics.width` 会把它剪掉，量一段纯空白得 0
（M3e 的 spike 在同一处栽过一次）。

判据 ⓪ 判在**画出来的像素**上（第一行最左边那道墨在第几列），不判我们自己算的盒子；
把 `foldedIndent` 改回恒 0，它当场红。

**其二：M2 的判据 ③ 自己拼 y，量到的是「段距 + 一页摊出来的行距」**。`LineRun.yOf`
的注释里写着「取 y 只能走这里」，而那条判据是 M3d 之前写的，没跟着改——参考机与
macOS 上都红着，主干上就红着。顺手修了（改走 `yOf`，期望值加上 `lineGap`）。
**这正是「四个消费方各写一遍就会各自漂」的第五个实例，而且漂的是判据自己。**

##### M4a 的判据（十四条，两处都跑）

⓪ 两端对齐不吃掉段首缩进（判在像素上）｜① 逐行点行首回的正是那一行的首字，
以及它的反面（命中 → 出框 → 落回同一行）｜② 命中走的是画出来的那份行 Paragraph｜
③ emoji 两半点下去同一个下标、切出来是完整的字｜④ 组合字符同样不被拆开｜
⑤ 跨行跨段出框：y 单调、都在视口里、区间外不出框｜⑥ 图片块点得中也出得了框｜
⑦ 分页与滚动同一个字同一个下标（逐行 × 三个 x），外加空块那条零高的线｜
⑧ 翻页之后同一个点给新页那个字｜⑨ 换字号重排之后同一个下标还指着同一段正文，
且在新矩形上点回去还是它｜⑩ 两端对齐开与关下**逐字**往返都闭合。

#### M4b — 选择与复制

**落地**（2026-09-02）。三份新文件：`reader_selection.dart`（选区、标记、以及
选择那件事的全部逻辑）、`reader_selection_layer.dart`（接手势的那一层与手柄、
工具条）、外加 `ReaderPainter` 多认一种东西。判据是 M4 那一套的 ⑪～⑯ 与
`app/test/reader_selection_test.dart` 的六条，macOS 与参考机各跑一遍全绿。

##### 一条贯穿全篇的口径：**标记只存 source 区间，不存任何几何**

页号、行号、矩形都是**这一刻**的排版算出来的——换字号、转屏、换阅读模式之后
全部作废，而 source 区间不变。所以「重排后批注位置稳定」不是要去修一堆坐标，
而是**压根不存坐标**：要画的时候现问一次 `rectsForSource`（M4a）。判据 ⑬ 判的
正是这一条：换字号重排之后 `selection` 一位没动、选中的正文一字不差，而矩形
换了位置（高度变了——一模一样反而说明它没跟着重排走）。

同一条口径下，**选择、高亮、下划线、搜索命中是同一个 `ReaderDecoration` 的四种
画法**：`ReaderPainter` 收一个 `ValueListenable<List<ReaderDecoration>>`，铺底色的
画在字下面（盖在字上面就是把正文调暗），下划线画在字上面。M4c 与 M4d 因此
只是往那张单子里多加几条，不动绘制。

##### 出入与理由

1. **选择逻辑做成 `ReaderSelectionModel`，不是 widget**。理由是判据：驱动一棵
   「与产品页长得差不多」的树，就是 phase3 记过的那种「裁判侧是桩」——判据全绿
   而产品是坏的。现在产品页与判据用的是**同一个**模型，widget 那两层
   （`ReaderSelectionGestures` / `ReaderSelectionLayer`）只负责接手势和摆手柄；
   阅读页因此只剩两件自己的事：边缘自动翻页往哪走（分页与滚动两条路的差别本来
   就在页这一层），以及复制完弹一句「已复制」；

2. **跨页选择靠「拖到边上自动翻页」**。一屏装不下的选区，手指伸不到下一页去——
   除此之外没有别的路。手指进到上下 48 px 的边带里就每 600 ms 翻一页，松手、
   离开边带、撤选就停。**不跨章**：换章会换掉整份正文，而选区是章内 source 下标，
   跨过去它就指向另一章的字了；

3. **翻页热区不撤选区，中间那个热区撤**。跨页选择要求翻页时选区还在；而「点一下
   取消」是所有人对选中态的预期，不该还要先去够那个「取消」按钮；

4. **手柄的圆点落在选区外侧，触摸区比圆点大一圈**。7 px 的圆点按不准，而按不准的
   手柄比没有手柄更让人恼火；圆点落在内侧会压住选区第一个/最后一个字；

5. **选区底色从正文色派生**（`ReaderPalette.selection` = 正文色 22% 透明），不另挑
   一个颜色。四套配色的底色明暗差得很远（纸白 vs 夜间），写死一个蓝会在其中一套上
   要么看不见、要么盖住字；

6. **长按选的是「词」，由 Paragraph 自己的 `getWordBoundary` 出**（底下是 ICU 的
   词边界），中文因此按词而不是按字选中——与系统输入框、浏览器同一套规矩。
   **段首缩进也是一个词**：长按它选中那两个全角空格是对的（§5.1 说它们是正文里的
   真字符），判据 ⑪ 因此把「有墨的词」单独数。

##### 欠着的账

- **词在行尾会被切断**。行 Paragraph 只装一行，跨行的那半个词它看不见，所以长按
  行末那个词偶尔只选到半个（拖一下就补回来）。这是按行绘制（§5.4）换来的，
  不值得为它把整段 Paragraph 重新养起来；
- ~~**阅读页那几行接线没有判据盖到**~~（**守卫修好了**，见 §8.6）。各自的本体
  本来就被判据以产品同款的树驱动过（比如判据 ⑯：长按 → 底色 + 手柄 + 工具条 →
  复制进剪贴板），缺的是「阅读页把它们摆在页边距之内、摆在翻页热区之上/之下」
  这一层 —— `ReaderPage` 要真书才开得起来（`rust.chapters` 走库），M4 这一套是离线的。
  **唯一那条端到端判据 `app_test.dart` 当时在主干上就是红的**，红在第一步。
  §8.6 把它整条重接到 N0/N1/N2 的新导航上，**macOS 与参考机上都跑通了全程**
  （**不是跳过**：两台都把三条跳过分支临时改成 `fail` 再跑一遍，一条都没触发）；
- **没有「全选本章」「分享」「查词」**，工具条上只有复制与取消。

##### M4b 的判据（⑪～⑯，两处都跑）

⑪ 长按选中的是一个词，落在正文上、切得出字｜⑫ 拖着扩选跨行跨段，选出来的正文与
`chapterText.substring` 一字不差，往回拖越过锚点会翻向｜⑬ **重排之后选中的正文
一字不差而矩形换了位置**｜⑭ **跨页**：拖到下沿自动翻页，选区跨过页界，这一页只
画得出一部分而复制出来的是整段｜⑮ 选区两端永远在 grapheme cluster 边界上
（一整行逐像素拖过去，切出来的正文里没有落单的代理）｜⑯ 产品路径：长按 →
底色（数着墨像素）+ 手柄 + 工具条 → 点复制 → 剪贴板里正是那段、选区当场撤掉。

#### M4c — 高亮、下划线与落库

**落地**（2026-09-02）。Rust 那边多一张表与一个落库面
（`store::highlight_store`）、engine 五个方法、FRB 五个口；Dart 那边一份
`reader_highlights.dart`（批注表、样式编解码、内存/落库两步走）与工具条上多的
两个按钮。判据是 M4 那一套的 ⑰～㉒ 与 `store` 的四条 Rust 用例，
macOS 与参考机各跑一遍全绿。

##### 表结构逐列对齐裁判的 `BookHighlight.kt`

`highlights` 表与 books/chapters/caches 同一条规矩（`db::SCHEMA` 的注释）：
列名逐字对齐上游 Room 实体，本 Phase 用不到的列照样建出来。这不是仪式——
data-storage-plan 把 `highlight.json` 排在「第二批，**需要先确定 Rubato 正文位置
模型**」，而 M4 正是那个模型：`chapterPos`/`chapterPosEnd` 就是章内 UTF-16 下标，
与 `durChapterPos` 同一把尺。

**`layoutTitleLength` 也写**。`durChapterPos` 的原点含章节标题那一块（§3.1 的
M2a），上游另存这一列好把标题那一截减回去
（`bodyPosition = durChapterPos - titleLength`）。不写它，一份 Rubato 的备份导进
裁判就整体错位一个标题的长度——**而那种错位看不出来是错位，只像「批注飘了一点」**。

##### 样式 JSON：认得两件，认不出的原样留着

上游那套 `HighlightStyle` 能组合九样（填色、字色、粗斜、下划线、删除线、外框、
着重号、阴影、字体路径）。Rubato 现在只画两件：`fill` 一层底色、`underline` 一道线。
**其余字段解出来放在 `extra` 里，编回去原样带着** —— 日后要画删除线，是多认一个
字段，不是改表，更不是丢掉别人备份里的东西。

两处易错，判据 ⑳ 都钉着：

- **颜色写成有符号 32 位**。上游那边是 Kotlin 的 `Int`，`0x80FFF176` 是负数；
  写成无符号，导进去就是另一个颜色；
- **`0` 是「这一路关着」，不是黑色**。写 `null` 会让上游解出默认值。

五个可选色号也逐值抄 `HighlightColors`——不是因为好看，是因为换一组颜色，
导出去就是五个陌生的色号。

##### 出入与理由

1. **先进内存、再写库；写失败当场撤回来**。划一道线要立刻看得见，而写库是几毫秒
   之后的事。静默留着一条没落库的批注，下次打开它就凭空消失了，而读者不会知道是
   哪一条；

2. **批注只按 `(bookUrl, chapterIndex)` 取**，不按章的 URL——换书源之后章的 URL
   会变，章号不会；

3. **移出书架时批注跟着删**，与删书在同一个事务里（所以 `HighlightStore` 借
   `BookStore` 那条连接，不另开一条：两条连接做不到同一个事务）。留着就是一堆
   再也打不开的孤儿；

4. **换章的入口收成一个** `_openChapter`。启动、翻过章界、目录、重试原来各调各的
   `_c.openChapter`——少接一处就是「那一章的批注不显示」，而这种漏最难发现：
   界面上什么都不缺；

5. **点在批注上不翻页**。批注画在正文里，而翻页热区盖着整屏；热区因此改用
   `onTapUp`，先把全局坐标换算回画布局部坐标问一句「这一下是不是点在批注上」。
   换算要一个挂在**页边距之内**的 `GlobalKey`——挂在页边距外面，算出来的坐标
   与 `rectsForSource` 差一个 padding；

6. **`ReaderPainter` 只收一张标记单子**，多张由 `MergedDecorations` 并好（批注在下、
   选区在上，所以选中一段已经高亮的正文时看得见选区）。让画笔收一串，等于把
   「谁盖谁」散到每个调用方去——那正是 M4a 收敛掉的那种形状。

##### 欠着的账

- **笔记（`note`）有列、有读写，但没有编辑界面**。菜单上只有换色 / 加下划线 / 删除；
- **没有「批注列表」那一屏**。`book_highlights` 这个口已经在了（按章号再按位置排），
  缺的是界面；
- **没有导入/导出**。表与样式 JSON 都是照着能互导的样子建的，但 Phase 4 那一步
  （读 legado 的 `highlight.json`）不在这个计划里；
- **重叠的批注只画得出叠加的样子，点中的是后划的那一条**。够用，但「两条重叠时
  想改前面那条」现在没有路。

##### M4c 的判据（⑰～㉒，两处都跑；外加 `store` 的四条）

⑰ 高亮多出一层底色、下划线多出一道线（判在像素上，且一道线比一整块底色少得多）｜
⑱ **重排之后批注不漂**：换字号，批注仍指着同一段正文而矩形换了位置｜
⑲ 点在批注上取得回那一条、点在别处取不到，后划的盖在前划的上面｜
⑳ 样式 JSON 与裁判互通：有符号颜色、认不出的字段原样留着、坏 JSON 退成空样式不抛、
`0` 是关着｜㉑ 落库：位置/章号/章名/**标题长度**/正文/笔记都写对了，写库炸掉当场
撤回来，空样式与倒序区间划不出东西｜㉒ 换章当场清空，连着换两章时过期结果不写进新章。

#### M4d — 章内搜索

**落地**（2026-09-02）。`reader_search.dart` 一份，加阅读页顶栏那一条搜索栏。
产物是一张标记单子——**与选择、批注同一套画法**，只是换了两个颜色
（普通命中一种、正看着那一个另一种，后者排在最后所以盖在上面）。

##### 出入与理由

1. **只搜这一章**。全书搜索要把每一章的正文都取回来（没缓存的还得联网），
   那是「后台任务 + 进度 + 可取消」那种形状，不是「在一段字符串里找子串」。
   这一条记在账上，不假装它做了；

2. **搜完跳到离当前位置最近的那一个**。读者在读第五页时搜一个词，
   结果被甩回第一页是最没道理的一种行为。跳转走
   `ReaderLayoutController.revealSource`——它只动页号/位移，与翻页同一档
   （§6.3 的热路径，不跨 FFI、不 layout），**不是 `openChapter` 的锚点那条路**；

3. **忽略大小写这件事先量过再敢做**。自然写法
   `haystack.toLowerCase().indexOf(needle.toLowerCase())` 在**裁判那一侧的语言里
   是错的**：Java/Kotlin 的 `toLowerCase()` 走完整 case mapping，土耳其语的 `İ`
   会小写成两个 code unit，于是小写版比原文长，回来的下标往后的全部偏——
   高亮框停在一句不相干的话上，而这看不出是搜索的锅。
   **Dart 走的是 simple case mapping**，一个 code point 换一个，长度恒等
   （实测 `İ` `ẞ` `ǅ` `Σ` `𐐀` `ﬁ` 一个都没变）。所以这里敢忽略大小写；
   长度那道门还是留着——它值一行，而 Dart 哪天换了，代价是「搜不到大写的」
   而不是「高亮画错地方」。判据 ㉔ 把两半都钉住；

4. **不逐字搜**。`TextField` 的 `onChanged` 每敲一个字就重扫整章，长章上那是
   几十万次比较乘以键盘节奏；提交（回车）与按上下键才搜；

5. **命中不重叠**（"aa" 在 "aaaa" 里是两处不是三处），**两端吸到 grapheme
   cluster 边界**——搜一个组合记号会切在 cluster 中间，画出来是半个字。

#### M4e — 翻页动画

**落地**（2026-09-02）。`page_transition.dart`（§4.2 的草图里点过名，M1 时记为
「留到 M4」）。四种：无 / 平移 / 覆盖 / 淡入，进阅读设置并落库。

**它守的是 M4 那一条：只消费相邻 PagePlan，不参与分页计算。** 落到代码上：

- 页号仍由 `ReaderLayoutController` 说了算，这一层只是**看见它变了**；
- 两页各走一遍 `forEachLineOnPage`（M4e 给控制器加的那条），那是**已经装好的**
  页表；`ReaderPainter` 只多认一个「画哪一页」的参数，画法一行没改；
- 动画期间不测量、不装页、不跨 FFI——判据 ㉙ 判的就是「页表还是同一个对象、
  `measureCount` 一动不动」；相邻页的行 Paragraph 由 `warmWindow` 在帧外备好（§6.3）。

##### 出入与理由

1. **只认相邻那一步**。跳章、目录跳转、搜索跳到十页之外都不 animate——
   那不是「翻了一页」，给它配一个翻页动画只会让人以为自己翻错了；

2. **覆盖那一种往回翻要反过来**。往回翻是「把盖上去的那一页揭走」，
   不是「拿旧的再盖一次」；

3. **滚动模式下这一项藏起来**，不是留一个拨了没反应的开关（那边没有「页」，
   位移由 `Scrollable` 自己出）；

4. **设置里存动画的名字，不存序号**。日后中间插一种，序号会把所有人的设置挪一位；

5. **`rectsForSource` 多了一个页号参数**。动画那一帧同时画着相邻两页，
   标记要落在**它那一页**上——否则动画里高亮会停在错的一页。

##### 判据侧的两条坑（都骗过一次）

- **`find.byType(Transform)` 数不出动画**：`MaterialApp` 自己就带两个，
  数出来永远是 2，于是「相邻翻页进了动画」和「跳远了没进动画」两条**同时全绿**，
  其实什么都没判。改成数「带页号的那种 `ReaderPainter`」——只有动画那一层给页号；
- **`pumpAndSettle` 在 live binding 上不等动画**。集成测试跑的是
  `IntegrationTestWidgetsFlutterBinding`（live），它只看 `hasScheduledFrame`，
  动画还没跑完就回来了。要显式 `pump(动画时长 + 余量)`，**再 pump 一帧**——
  动画结束那一帧只把 `setState` 排上，要下一帧才重建。

##### M4d / M4e 的判据（㉓～㉛，两处都跑）

㉓ 找全、不重叠、按位置排｜㉔ 忽略大小写不许把下标带偏（钉住「Dart 的小写化
保长度」这个前提）｜㉕ 命中两端吸到 cluster 边界｜㉖ 跳到离当前位置最近的那一个，
上一个/下一个绕回去且真的换页｜㉗ 当前命中与其余不同色且盖在上面，换词/清掉跟着变｜
㉘ 画某一页：越界给不出行，给对了与「当前页」逐位一致｜㉙ **动画期间不测量、
不装页**｜㉚ 只认相邻那一步，关掉动画就是原来那一层｜㉛ 动画那一帧真的画了两页。

### 8.6 M4 收盘之后的清账（2026-09-02）

M0～M4 收完之后把 M1～M4 记下的四十来条账**逐条对着代码核了一遍**，而不是照着
台账直接动手。核完的结果分四堆，这一节记前两堆干了什么、后两堆为什么不干。

**核账本身就翻出三条记错了的**：

1. **「M4 命中测试要按需重建整段 Paragraph」这笔取舍从来没有被兑现过**。M4a 走的是
   按行 Paragraph + 缓存，整段的一次都没建。而且**必须**这样 —— 整段 Paragraph 是
   `TextAlign.start`，拿它反算居中的标题行会偏出几十像素。这条账把前提当成了负担；
2. **「标题没有居中」M3c 就还了**，台账上还挂着；
3. **`app_test.dart` 比记的烂**。台账记的是「N0 把搜书从 FAB 挪走了，判据还按老入口找」
   —— 那只是第一步。往下 N1/N2 又把书架换成了封面网格、把「点书直接进阅读页」改成了
   「点书进详情页再点开始阅读」。修它不是改一个 finder，是把整条路重接一遍。

#### 还了的

| 账 | 记在 | 收法 |
|---|---|---|
| 端到端守卫红着 | M4b | `app_test.dart` 重接到 N0/N1/N2 |
| 无障碍标签逐页拼 | M1 | `ReaderPainter.semanticsBuilder`，按段一个节点 |
| 图片不可点、不能看大图 | M2b | 热区那条链上多一个分支 + `ReaderImageViewer` |
| 取图失败不重试（**只开手动那条**） | M2b | `ReaderImageStore.retry` |
| 拉丁文 justify 拉的是词距还是字距，没验 | M3c | M3 真机第 ⑰ 条 |
| 没量宽度表按 (字体, 字号) 缓存的命中率 | M3e | spike 第 ⑥ 条 |

**判据先行**（§12 的第 1 条）：这一轮加了 M2 的 ⑧ 加强、M3 的 ⑰、M3e 的 ⑥、
`reader_images_test` 两条、`reader_image_viewer_test` 一条，外加重接的 `app_test`。

**两台都过了**（2026-09-02 同日补跑，参考机 HITV205N / Android 11 / `c6bbcb77`）：

| 套 | macOS | 参考机 |
|---|---|---|
| host（`flutter test`） | 105 条全过 | 同一份（不挂设备） |
| M1 | 8 条 | 8 条 |
| M2（含加强过的 ⑧） | 18 条 | 18 条 |
| M3（含新增的 ⑰） | 17 条 | 17 条 |
| M3e spike | 只报数 | 只报数（① 那条真的跑了 3 分 10 秒） |
| M4 | 35 条 | 35 条 |
| `app_test` 端到端 | 全程 14 s | 全程 43 s |

**两处两台的数不一样，都不是红**：

- **⑰ 拉丁 justify**：macOS 拉开 7 处词距 / 24 个字词内没动，参考机是 8 处 / 26 个。
  系统字体不同 ⇒ 同一行断在不同的词上。这条判的是**结构**（词距变宽、词内不动），
  不是那两个数 —— 判死数字就会在换一台机器时红，而那不是产品的错；
- **⑥ 宽度表**：macOS 偏差 0.000%，参考机 0.058%。结论不变（键里不放字号），
  但那 0.058% 是什么，写在 M3e 那条账上。

#### 三处「判据会骗人」的新形状

这一轮又踩了三个，形状都与 M2/M3/M4 记过的同源，但入口是新的：

1. **finder 找的那一层不是产品放东西的那一层**。`CustomPainterSemantics` 造的
   `SemanticsNode` 不属于任何 Element，而 `find.bySemanticsLabel` 读的是
   `element.renderObject.debugSemantics?.label` —— 它只看得见画布**自己**那个边界节点，
   标签全在子节点上。照它写，判据会以为「读屏什么都听不见」。要判就得自己走一遍树
   （`test/reader_semantics_probe.dart`）；
2. **两次问的是同一个对象**。`ParagraphBackend.lineParagraph` 的缓存键是
   `(blockId, lineIndex)`，**`lastLine` 不在键里** —— 判据想拿「同一行不拉伸时」当参照，
   再问一次拿回的是刚才那份拉伸过的。于是「词内位置没变」恒真、「词距变宽了」恒假。
   ⑰ 第一版正是这么红的，而它**红得很像产品的错**（「justify 没生效」）；
3. **`print` 在 `flutter test -d macos` 里不回传**。`app_test` 的三条跳过分支各打一句
   日志，而那三句在 runner 的输出里一个字都看不到 —— 于是「全程跑通」与「第一步就跳过」
   **在输出上完全一样**，都是 `All tests passed!`。判它跑没跑到，得把跳过临时改成
   `fail` 再跑一遍。**凡是「判据里有跳过分支」的，都要有一次这种反向验证**，
   否则那条判据可能已经空转很久了。**两台各验了一次** —— 参考机上尤其要验：
   那几个源在手机上被 DNS 拦过（见界面那条线的记录），「跳过」在那台机器上
   是真会发生的事，不是理论可能。

#### 顺带把一条结论往前推了一格

M3e 的 ⑥ 量出「宽度归一到每 em 之后跨 9 档字号偏差 0.000%」，意味着字符宽度表的键里
**不用放字号**。M3e 收盘时算的是「换字号整张作废，而字号是滑块」，现在那笔一次性开销
（参考机 0.37–1.0 s）**一辈子只付一次**。悬挂那条路的成本比 M3e 判的时候低一个数量级
—— **这不改 M5 的结论，但改了它的算式**，M5 真开门时要按新的数算。

#### 没还的，以及为什么

**A. 是决策，不是债**（这一轮把它们的标签改了 —— 挂在「欠账」下面会让人一次次
重新翻出来问，而它们的理由都已经写全了）：

| 条目 | 记在 | 为什么不是债 |
|---|---|---|
| 字号重排到首屏压线 | M2a | §10 已判「按原方案做更差」，门槛留到有采样时再定 |
| `<` `>` 仍会出现在行首/行尾 | M3a | 要它们的代价是整条 §7.2 第二阶段 |
| 压缩表没抄 | M3a | 消费者在 M3d，而 M3d 把压缩并到了 M5 |
| 标点悬挂与压缩 | M3e | M5 那道门后面 |
| 词在行尾会被切断 | M4b | 按行绘制换来的，不值得为它把整段 Paragraph 养起来 |
| 滚动不记像素、`scrollProgress` 按像素 | M2c | 像素位置换个字号就没有意义 |
| 没有 golden（两处） | M3a/M3d | 要挡的是算得出来的不变量，不是像素比对 |
| `ImageUtils.decode` 没做 | M2b | 不做比留个半截实现好 |
| 混排只判结构不判好看 | M3a | 不是这一版要回答的问题 |
| `min` 不可调、`MAX_BOTTOM_GAP_RATIO` 是看出来的 | M3b/M3d | 要调得先有更多语料与屏幕 |
| 翻页那一帧贵了 0.4 ms | M3c | 在预算里（门槛 8 ms） |

**B. 有前置，不该在这条线上还**：

- **`caches` 的容量与淘汰**（data-storage-plan 的 P2）—— 图片字节持久缓存与 §6.2 的
  L2 文档预取都压在它下面。那是**存储那条线**的账，在这里还它等于绕过它的域拆分；
- **全书搜索**（M4d）、**滚动下连章读**（M2c）—— 前者是「后台任务 + 进度 + 可取消」，
  后者要改 `durChapterPos` 的口径（它是**章内**下标）。两件都够单开一片，不是清账；
- **批注列表那一屏 / 笔记编辑界面 / 全选本章**（M4b/M4c）—— 管道全通了，缺的是界面。
  它们是**产品面**，该跟着界面那条线走，不该塞进排版专项的清账里。

**C. 真欠着但很小，没人撞得上**：标题 + 一行正文的页会被孤行控制排空（极窄视口）、
「做不到」的口径写在两处、重叠批注点中的是后划的那一条、M3e spike 的语料只有 78 个
不同字。留着。

### M5 — 原生 Rust 渲染决策门

**M3e 把两件事并到了这道门上**（2026-09-01）：标点悬挂与标点压缩。
它们不是「还没做」，是**在当前绘制模型下过不去**——压缩要逐字画（参考机上是
按行画的 5.8 倍、§10 门槛的 2.4 倍），悬挂要 Rust 拿字符宽度表自己断行
（一次性 0.37–1.0 s 建表，且连写脚本必须退回 ICU）。完整的实测与理由在 M3e。
这道门要判的因此不只是「Rust 渲染值不值」，还有「中文排版最后那两项值不值」。

**2026-09-02 清账那一轮把这道门的算式改了一格**（§8.6）：M3e 收盘时以为字符宽度表要按
(字体, **字号**) 缓存、而字号是滑块，于是那笔一次性开销（参考机 0.37–1.0 s）每滑一下付
一次。实测下来宽度归一到「每 em」之后跨 9 档字号偏差 **macOS 0.000% / 参考机 0.058%**
—— 键里不用放字号，那笔开销**一辈子只付一次**。**结论没变（仍然默认不启动），
变的是悬挂那条路的成本**：真开门时按新的数算，别照着 M3e 收盘时那一版。

参考机那个 0.058% 同时给这道门添了一条要判的：它意味着 Rust 拿这张表断出来的行，
与 Flutter 自己断的**可能差一个字**（只在恰好卡在行末边界上时）。这条要和 M3e ② 的
「连写脚本上和 ≠ 整体」一起判 —— 两条都是同一个问题的不同侧面：
**这张表在多大范围内可以当成真相**。


M5 默认结论是“不启动”。只有满足任一条件才做 spike：

- 混合方案在 M0 锁定的参考设备上持续达不到性能门槛；
- 产品明确要求跨平台接近像素一致；
- 必需中文排版规则无法通过 Paragraph + PageComposer 正确表达；
- Paragraph 的内存或生命周期问题有可复现证据且无法规避。

spike 范围仅包括：

- Parley 或等价 Rust shaping/layout；
- 固定字体与系统 fallback 两条路径；
- 一页纹理的 Android/macOS/Windows 接入；
- emoji、Bidi、选择命中、DPR 变化和纹理释放；
- 与混合方案相同 benchmark 的 A/B 数据。

spike 通过评审前，不向 workspace 引入正式字体渲染依赖，不改产品阅读页。

## 9. 测试体系

### 9.1 Rust 纯函数测试

- 页面范围连续、单调、无重叠、无遗漏；
- 任意非空页面至少推进一个 source offset；
- 空章、零宽/零高视口、单行高于页高时不死循环；
- 锚点总能解析到有效页或明确回退；
- page composer 对同一输入确定性输出；
- session/generation 过期结果永不提交；
- property test 生成随机 line metrics 和 block 序列。

**M1 落地**:上面每一条都有用例(`reader-doc` 10 条、`reader-layout` 18 条、
`engine::reader` 6 条)。另外多出两条计划里没写、但实现时发现必须钉住的:

- **前缀装页与全量装页逐页相同**(除最后一页)。首屏只量到锚点页够用的块就先画,
  靠的就是这条 —— 不成立的话,整章补齐之后先画的那几页会跳位置;
- **块表的无缝性**:`block[i].source_end == block[i+1].source_start`,末块盖到
  `source_utf16_len`。分页的无缝性建在它上面,而它自己也有一条随机正文的 property test
  (换行、空行、emoji、缩进混着来)。

**M2a 落地**又多出四条(`reader-layout`)加四条(`reader-doc`):

- **块前间距在页顶折叠**,而且是**每一页**的页顶都折叠 —— 不是「第一页才折叠」。
  少了这一条,每页顶上白空一个段距,还让每页少装一行;
- **标题靠自己的块前间距拉开**:同一份输入下标题下间距与段距各落各的位,
  而装页器全程不认识「标题」—— 它只看到一串算好的数;
- **块前间距非法(负数 / NaN / inf)当场拒绝整次布局**,不 clamp。静默改成 0
  只会让「Dart 那边的样式算出了 NaN」变成「排版偶尔差一截」;
- **标题成块**的四条:标题成为第一个块且计进章长、标题里的换行收成空格(恒为一个块)、
  空白标题退回无标题(与不带标题那条路逐位相同)、标题与正文都空仍是一个块。

### 9.2 Dart/Flutter 测试

- `ParagraphBackend` 缓存命中、失效与 dispose（用 `Paragraph.debugDisposed` 断言，不数 event loop）；
- 同一 block/layout key 只 layout 一次；
- PagePlan 的行级 Placement 绘制（clip/translate 到 `line_start..line_end`）；
- 点击和选择 offset 映射；
- widget test 使用仓库内固定字体，避免宿主字体漂移；
- golden 按平台维护，不做跨平台字节比较。

**M1 落地**的三处（都在库里，不需要真机）：

| 用例 | 判什么 |
|---|---|
| `app/test/reader_layout_wire_test.dart` | 三段线格式的 Dart 这一半，**逐字节**断言。两侧对着同一份写在注释里的格式实现，谁改一侧不改另一侧当场就红 |
| `app/test/reader_paragraph_backend_test.dart` | 行区间无缝覆盖整块；空块也有一行；几何键/绘制键分家；`retainLines` 之外当场释放；**按行重建的 Paragraph 与整段里的那一行同宽**（这条是「按行画」这条路的地基） |
| `app/integration_test/reader_layout_m1_test.dart` | 要真 Rust 的那些：分页不变量、热路径、200k 曲线、锚点、过期代号、资源释放、**着墨像素**、新旧对照。语料离线可复算，经 `prepareChapterText` 这个诊断口进 Rust，不挂网络 |
| `app/integration_test/reader_layout_m1_bench_test.dart` | 产品路径基线（新旧同机同轮），只报数不设门，数字进 §10 |
| `app/test/reader_settings_test.dart` | 设置的编解码。规矩与线格式**相反**：线格式对不上整份拒绝（两侧代码不同步 = bug，当场炸），设置读不出来则**逐项兜底**（多半是加了一项或换了版本，把字号退回 18 比让人打不开阅读页强）。另判越界值被夹回范围 —— 一份手改坏的设置不该排出 0 高的行 |
| `app/test/reader_settings_sheet_test.dart` | 面板在三种屏宽下不溢出（真机上溢出只是屏幕角上一条黄黑杠，没人截图就发现不了；`flutter_test` 会把它判成失败），字号到边界后按钮变灰 |
| `app/integration_test/reader_layout_m2_test.dart` | M2a 的七条：标题成块与分页不变量、标题按标题样式量也按标题样式画、段距进页表且页顶折叠、**换配色只重绘不重排**、换字号/转屏后锚点不丢、设置落库往返、滑块松手才提交；M2b 的四条（⑧～⑪）：img 自成一块且进页表与读屏标签、**图片到齐只重装页不重新测量**且读者不被扔走、超高图片压到视口以内且诊断干净、取不到的图按失败档占位不把正文拦住。取图那一步是喂字节的桩，解码那一步不是（走真的 `decodeImageFromList`）；M2c 的七条（⑫～⑱）：换模式只重装页且共用同一份文档、滚动下 `durChapterPos` 是第一个完整可见字**且往返稳定**、分页↔滚动来回不丢位置、**视口高度变了也只重装页**、窗口连续缩放、连着切模式旧页表不覆盖新设置、**真的拖一下**画布跟着换 |
| `app/lib/reader/reader_surface.dart` 的 `ReaderScrollView` | 滚动那一半的视图。判据 ⑱ 直接 pump 它并**真的拖一下** —— 位移归 Flutter 的 `Scrollable`、controller 是它的镜像，那种「两边各滚一次」的打架只有真拖才看得出来 |
| `rust/crates/reader-layout` 的 `孤行控制*` 九条用例 | M3b：两行段落不拆、页尾孤行往回拉、页首孤行整段下推、续排块不受页首那条约束、页还空着时下推没有去处、滚动下是空操作、不动图片块、前缀装页仍逐页相同、随机输入下规则真的兑现。**旧口径（`min_split_lines: 1`）留着**，M1/M2 那些逐页对照的用例继续跑在它上面 |
| `app/integration_test/reader_layout_m3_device_test.dart` 的 ⑮⑯ | M3d：非末页要么正文下缘**贴着页底**，要么它每一道行距要撑的量超了上限（两档之外没有第三种）；末页没被摊；行序不乱。**取 y 只能走 `LineRun.yOf`**——摊了行距之后 `originY + line.top` 只对本段第一行成立，绘制/备行/命中/读屏各写一遍就会各自漂 |
| `app/integration_test/reader_layout_m3_device_test.dart` 的 ⑫⑬⑭ | M3c：换两端对齐**只重绘**（不重新测量、页表逐位不变）、非末行真的铺满而末行不拉伸（759 条）、标题居中。判的是**画出来的那一份**（行 Paragraph），不是设置里的一个 bool |
| `app/integration_test/reader_layout_m3_device_test.dart` 的 ⑨⑩⑪ | M3b 的真机口径：五种视口 × 23 个被拆开的块，页底与次页顶都 ≥ 2 行；分页不变量不破；窄到 80 px 的视口也没排出空页（「整块下推」若不看「页还空着吗」，这一条会挂在死循环或一串空页上） |
| `app/test/reader_layout_m3e_spike.dart` + `_test.dart` + `integration_test/reader_layout_m3e_device_test.dart` | M3e 的 spike（§7.2 要求的那一个）。**只报数不设门**：逐字宽度三条路的耗时、逐字和是否等于整行宽度（七种脚本）、任意切点重建是否改字形、逐字画一页有多贵、标点的墨水盒。结论写在 M3e |
| `app/test/reader_layout_zh_rules.dart` + `_test.dart` + `integration_test/reader_layout_m3_device_test.dart` | M3a 的中文排版规则：禁则表逐字过 ICU（一个都不该漏）、禁则语料整份 0 违例、**豁免只在「无处可断」时给且判据不恒真**、混排下行区间无缝/单调/不切开 grapheme cluster、同一份输入断出同一份行、**行区间与 `getLineBoundary` 逐条一致（含 RTL）**、行区间无缝盖住整块正文、取行区间三条路的耗时。判据走产品那一条（`ParagraphBackend.lineBoxesOf`），真身由 `getLineBoundary` 出——**判据侧不另写一份** |
| `app/test/reader_selection_test.dart` | M4b 的**纯规则**:`start/end` 规范化而 `anchor/extent` 保留方向、拖过头翻向且锚点不动、抓一个手柄固定另一端(拖过对面也不塌成空)、标记只带 source 区间与画法 |
| `app/test/reader_hit_test_test.dart` | M4a 那几条**纯函数**:向前吸到 grapheme cluster 边界(代理对、组合记号、ZWJ 序列、越界)、回退一整个 cluster、前导缩进折进占位符之后的下标换算两条互为逆。不要真 Rust 也不要真字体 —— 它判的是「算错一位会怎样」 |
| `app/integration_test/reader_layout_m4_test.dart` | M4d/M4e 的九条(㉓～㉛):搜索找全不重叠、**忽略大小写不许把下标带偏**、命中吸到 cluster 边界、跳到最近的那一个且能绕、当前命中不同色且盖在上面;画某一页与「当前页」逐位一致、**动画期间不测量不装页**、只认相邻那一步、那一帧真的画了两页 |
| `rust/crates/store` 的 `highlight_store` 四条 | M4c 的落库面:按章取回来且按位置排、**同一毫秒里连划两段不会互相顶掉**、改样式不动位置(`layoutTitleLength` 不许丢)、整本删干净 |
| `app/integration_test/reader_layout_m4_test.dart` | M4c 的六条(⑰～㉒):高亮与下划线画在像素上、**重排后批注不漂**、点在批注上取得回那一条、样式 JSON 与裁判互通(有符号颜色 / 认不出的字段原样留着 / 坏 JSON 不抛)、落库两步走与写失败撤回、换章清空且过期结果不落地;M4b 的六条(⑪～⑯):长按选词、拖着扩选跨行跨段与翻向、**重排后正文一字不差而矩形换位**、跨页(拖到下沿自动翻页)、两端永远在 cluster 边界、产品路径长按到剪贴板；M4a 的十四条：命中与出框互为逆、命中走**画出来的**那份行 Paragraph（与整段那份必须给出不同的字）、emoji 与组合字符不被拆开、跨行跨段出框、图片块、分页与滚动同一个字同一个下标、翻页/换字号之后仍闭合、两端对齐开关下逐字往返；外加 ⓪ **两端对齐不吃掉段首缩进**——判在画出来的像素上 |
| `app/test/reader_images_test.dart` | 图片那一块占多高 —— 它决定分页，所以必须能脱离网络单测：三档占位高度、超高图片压到视口以内、小图不放大；以及状态表（同一 URL 只取一次、取完叫一声、取不到/解不开都落在失败档、换章之后到货的当场丢掉） |

绘制那条（clip/translate 的 golden）**M1 没做**：M1 的绘制只有「把行 Paragraph 画到
`origin_y + line.top`」一句，而它对不对已经由「按行重建与整段同宽」那条用例判死了；
golden 等 M2 的样式与图片进来再建，那时它才开始能挡住东西。

**M2b 也没建 golden**。图片那一块现在有三个可画的状态（占位框 / 图 / 失败框），
听上去正是 golden 该挡的东西 —— 但真要挡住的那条是「**画出来的图不许超出页表给的那一格**」
（图片取到的那一刻它的理想高度就变了，页表要等下一次重装页才跟上，中间那几十毫秒里
按理想尺寸画就会压到下一段正文上）。那是一条**算得出来**的不变量，不是像素比对：
绘制侧按 `Rect.fromLTWH(originX, originY + line.top, backend.width, line.height)` 去 contain，
天然不会越界。为它建三平台 golden，挡住的是配色和圆角，不是这条。

### 9.3 集成测试

扩展现有“书架 → 阅读 → 翻页 → 进度”路径，增加：

- 长章连续翻 100 页；
- 字号变化后仍位于原正文附近；
- 横竖屏/桌面窗口缩放；
- 跨章上一页/下一页；
- 图片加载失败与重试；
- 快速连续修改设置，旧 generation 不回写；
- 退出阅读页后 Paragraph、图片和 layout session 释放。

### 9.4 为什么没有 Legado 差分套

其余各套（fetch / rule-engine / js-host / webview …）挂真身对照，是因为书源生态定义了
正确性。排版的正确性由中文排版惯例定义，比对上游输出既不可复算（字体度量不同），
也不说明谁对谁错。防回归靠 §9.1 的规则用例和 §9.2 的 golden，不靠第四个 harness。

## 10. 性能门槛

M0 实测后锁定最终数字；实施前暂用以下门槛，任何调整必须在 benchmark 报告解释：

| 指标 | 暂定门槛 |
|---|---|
| 20k UTF-16 冷首屏 | 参考机 p95 ≤ 150 ms（只量到锚点页够用的 block，不等整章） |
| 100k UTF-16 整章补齐 | 参考机 p95 ≤ 1.2 s，后台分批完成，不阻塞首屏与翻页；曲线近似线性 |
| 已缓存普通翻页 | 100 次翻页无 layout/FFI，build+raster p95 ≤ 8 ms |
| 一页绘制 | 按行绘制，与段落长度无关；参考机实测 0.34 ms 录制，门槛 p95 ≤ 2 ms |
| 字号/视口重排 | 当前页优先返回；p95 ≤ 150 ms |
| 文字布局缓存 | 当前章 + 相邻页缓存的 Paragraph 数与字符总量上限（M0 定值）；原生内存用 DevTools/Instruments 抽测记录，不作自动判据 |
| 生命周期 | 退出阅读页后无主 Paragraph 全部 `debugDisposed`，layout session 释放 |

**当前实现基线**（2026-09-01，单段纯 CJK，视口 400×700；`host` = flutter_tester，
`device` = HITV205N / Android 11 / arm64，profile）：

| 单章 UTF-16 | host 当前 `_paginate` | host M1 路径 | **device 当前 `_paginate`** | **device M1 路径** | device 倍数 |
|---|---|---|---|---|---|
| 10 000 | 26.8 ms | 3.4 ms | 412 ms | 42.5 ms | 10× |
| 50 000 | 463 ms | 20.6 ms | 9 736 ms | 245 ms | 40× |
| 200 000 | 7 547 ms | 136 ms | **159 330 ms** | 1 324 ms | 120× |

「M1 路径」= 建整段 Paragraph + 逐行 `getPositionForOffset`，即 `MeasureBatch` 的 Dart 侧
成本，不含 Rust 装页（纯函数，量级可忽略）。当前实现在真机上 200k 单章要 **159 秒**——
§2 说的「不作为长期排版架构」在这台机器上是「根本跑不完」。原定「至少快 3 倍」的判据
按真机重述：50k 以上必须快一个量级，只快几倍说明装配方式写错了。

真机数据还定了两件事：

- 整章测量本身就是几百毫秒量级（200k 单段 layout 676 ms），所以 §6.1 的测量必须**分批**，
  首屏只量到锚点页够用的 block，不能先量完整章；
- 这台参考机是电视盒子，CPU 弱于同期手机，当作**悲观下限**用。手机上的数应更好，
  但门槛按这台定，过了这台就不会在别处翻车。

benchmark 必须区分：正文获取、Rust 文档构建、FFI 传输、Paragraph 创建/测量、Rust 分页、
首屏绘制、整章补齐和图片解码，禁止只报一个总耗时。

### M1 落地后的产品路径基线（2026-09-01）

harness `app/integration_test/reader_layout_m1_bench_test.dart`，**新旧同机同轮**，
走的是真正的 `ReaderLayoutController`（不是把流程再抄一遍）。语料由
`app/assets/reader-layout/plain.txt` 复制拼接到目标长度；视口 360×640，字号 18/行高 1.7。

参考机 **HITV205N**（Android 11 / arm64 / 电视盒子，作悲观下限），**profile 模式**
（`flutter drive --profile`；`flutter test` 走的是 debug，实测慢约 1.5–2 倍，别混着比）：

| 单章 UTF-16 | 旧：整章分页 | 新：冷首屏 | 新：整章补齐 | 新：翻页(帧内) | 新：帧外补邻页 | 新：字号重排到首屏 |
|---|---|---|---|---|---|---|
| 20 000 | 3 288 ms(52 页) | **58 ms**(先出 3 页) | 124 ms(51 页) | 1.82 ms/页 | 12.3 ms | 99 ms |
| 50 000 | 21 041 ms(129 页) | **28 ms** | 191 ms(128 页) | 1.81 ms/页 | 26.3 ms | 169 ms |
| 200 000 | (见上表的 M0 实测) | **62 ms** | 742 ms(504 页) | 1.70 ms/页 | 23.7 ms | 163 ms |

macOS(M 系列,debug)同一份 harness：

| 单章 UTF-16 | 旧：整章分页 | 新：冷首屏 | 新：整章补齐 | 新：翻页(帧内) | 新：字号重排到首屏 |
|---|---|---|---|---|---|
| 20 000 | 264 ms | 29 ms | 46 ms | 0.24 ms/页 | 21 ms |
| 50 000 | 1 727 ms | 15 ms | 52 ms | 0.19 ms/页 | 36 ms |
| 200 000 | — | 39 ms | 190 ms | 0.16 ms/页 | 35 ms |

读法：

- **冷首屏与章长基本无关**(参考机 28–62 ms),因为首屏只量到锚点页够用的块就先装一次页
  (§6.1 第 2/4 步);旧那条没有「先出首屏」这回事 —— 算完整章才画,50k 要 **21 秒**。
  M0 定的「50k 以上必须快一个量级」在这里是**快两到三个量级**;
- **整章补齐近似线性**:参考机 124 → 191 → 742 ms(20k → 50k → 200k),而旧实现
  20k → 50k 是 3.3 → 21 秒,2.5 倍的正文换来 6.4 倍的时间 —— 那条曲线是二次的;
- **翻页帧内 1.7–1.8 ms**(参考机),含走一遍本页摆放把每行 Paragraph 取出来;
  暂定门槛是 build+raster p95 ≤ 8 ms,这一项在预算里;
- **字号重排到首屏 99–169 ms**,暂定门槛 p95 ≤ 150 ms —— **50k 那一档的 169 ms 压线偏出**。
  它比冷首屏慢是有原因的:重排的锚点在读者当前位置(基线里是翻了 100 页之后),
  锚点之前的块必须先量完才知道页号,这是「按 offset 恢复位置」的固有代价,不是装配问题。
  真要压,得给锚点之前的块留一份「上一次的行度量」按几何键复用 —— 记在 M2 的账上,
  不在 M1 硬凑;
- **帧外补邻页 12–33 ms**(参考机)。它在 post-frame 回调里跑,不占翻页那一帧;
  但连着快速翻页时它会一帧接一帧地做,是 M2 要盯的下一个点。

上表里的门槛**暂不改动**:一台参考机、每档一次采样,不够锁 p95。M2 收样本之后再定。

### M2a 之后复测(2026-09-01,同一台参考机、同一条 `flutter drive --profile`)

| 单章 UTF-16 | 新:冷首屏 | 新:整章补齐 | 新:翻页(帧内) | 新:帧外补邻页 | 新:字号重排到首屏 |
|---|---|---|---|---|---|
| 20 000 | 54 ms(先出 3 页) | 121 ms(51 页) | 1.86 ms/页 | 11.4 ms | 101 ms |
| 50 000 | 28 ms | 192 ms(128 页) | 1.85 ms/页 | 23.7 ms | **171 ms** |
| 200 000 | 59 ms | 737 ms(504 页) | 1.74 ms/页 | 24.8 ms | 167 ms |

**每一档都落在 M1 那次的噪声里**(±4 ms 以内)。这符合预期:M2a 没动测量那条路 ——
仍是「每块建一次整段 Paragraph + 逐行 `getPositionForOffset`」,多出来的只有
「一个标题块」和「每块算一个 `space_before`」,两者都不在量级上。

**字号重排到首屏那一档仍然压线**(50k 是 171 ms,门槛 150 ms),M1 记的那笔账
**没有还** —— 真因与当时设想的解法(装页器支持从锚点块起装**后缀**)写在 M2a 的欠账里。

### M2c 之后复测（2026-09-01，同一台参考机、同一条 `flutter drive --profile`）

| 单章 UTF-16 | 新：冷首屏 | 新：整章补齐 | 新：翻页（帧内） | 新：帧外补邻页 | 新：字号重排到首屏 |
|---|---|---|---|---|---|
| 20 000 | 56 ms（先出 3 页） | 122 ms（53 页） | 1.78 ms/页 | 12.2 ms | 99 ms |
| 50 000 | 30 ms | 186 ms（131 页） | 1.74 ms/页 | 22.8 ms | **162 ms** |
| 200 000 | 58 ms | 718 ms（516 页） | 1.60 ms/页 | 22.7 ms | 156 ms |

M2c 新增的那两条 —— 它们**只重装页，不重新测量**：

| 单章 UTF-16 | 换阅读模式（分页↔滚动） | 收起工具栏（视口矮 120） | 测量次数变了吗 |
|---|---|---|---|
| 20 000 | 3 ms | 1 ms | 没变 |
| 50 000 | 5 ms | 7 ms | 没变 |
| 200 000 | **8 ms** | **16 ms** | 没变 |

读法：

- **换模式与收起工具栏都是个位数到十几毫秒**，而同一台机器上「整章补齐」是
  122 / 186 / 718 ms。收起工具栏在 M2c 之前走的正是后者那条 —— 切一次工具栏
  白重测一遍整章。这就是那笔账的兑现；
- **「测量次数变了吗」这一列是判据的一半**。数字快不代表走对了路：它可能只是
  「这一次没测完」。这一列在第一版 harness 里报的是「变了(不该)」，追下去发现是
  **量表的写法错了**（字号那条量的是「到首屏」，那时整章还在后台接着测，
  M2c 这两条就排在它后面），不是产品错了。改成先等 `complete` 再开表之后，
  三档都「没变」，而 200k 那档从 583 ms 掉到 8 ms —— 中间那 575 ms 是真的白工
  （见 M2c 的「benchmark 抓的那个」）。**benchmark 也会骗人，而且骗法与判据同源**：
  等待条件挑错一步，量到的就不是你以为的那件事。

### M3a 之后复测（2026-09-01，同一台参考机、同一条 `flutter drive --profile`）

| 单章 UTF-16 | 新：冷首屏 | 新：整章补齐 | 新：翻页（帧内） | 新：帧外补邻页 | 新：字号重排到首屏 |
|---|---|---|---|---|---|
| 20 000 | 51 ms（先出 3 页） | **103 ms**（53 页） | 1.73 ms/页 | 11.2 ms | 93 ms |
| 50 000 | 43 ms | **166 ms**（131 页） | 1.74 ms/页 | 23.4 ms | **122 ms** |
| 200 000 | 55 ms | **565 ms**（516 页） | 1.56 ms/页 | 24.1 ms | **115 ms** |

M3a 只改了一处：取行区间从 `getPositionForOffset` 换成 `getLineNumberAt`（§5.3）。
改它的理由是**正确性**（RTL 段落行区间整份偏移），性能是搭上的：

- **「字号重排到首屏」那笔压线的账还了**。50k 那档 162 → **122 ms**（门槛 150 ms），
  200k 156 → **115 ms**。这笔账从 M1 记到 M2c，两次都当成「按 offset 恢复位置的固有
  代价」；真因有一半在这里——`getPositionForOffset` 每行的成本随**行数**线性，
  重排要把锚点之前的块全量完，长章上它是实打实的一截；
- **整章补齐每档降一到两成**（122→103 / 186→166 / 718→565 ms），曲线仍近似线性；
- **冷首屏与翻页落在噪声里**（首屏只量两三屏的块，行数小，那条路本来就不吃这个成本）。

一条教训记在这儿：**这笔账被「解法看着不划算」挡了两片**（M2a 提出后缀装页、
M2c 论证它更差），两次都没有回头问「那 169 ms 到底花在哪儿」。真因不在装页那一侧，
而在测量那一侧的一个 API 选择上——而那个选择当年是有实测支撑的（M0 的两条路对照表），
只是**没量第三条**。

### M3b 之后复测（2026-09-01，同一台参考机、同一条 `flutter drive --profile`）

| 单章 UTF-16 | 新：冷首屏 | 新：整章补齐 | 新：翻页（帧内） | 新：帧外补邻页 | 新：字号重排到首屏 |
|---|---|---|---|---|---|
| 20 000 | 52 ms（先出 3 页） | 105 ms（53 页） | 1.73 ms/页 | 10.6 ms | 86 ms |
| 50 000 | 25 ms | 146 ms（133 页） | 1.72 ms/页 | 22.5 ms | 119 ms |
| 200 000 | 58 ms | 564 ms（524 页） | 1.53 ms/页 | 24.1 ms | 115 ms |

**每一档都落在 M3a 那次的噪声里**。这符合预期：孤行控制是 `compose` 里的几个比较，
而 `compose` 是纯函数、量级本来就可忽略。变的只有**页数**（131 → 133、516 → 524）——
把行往下赶就是多出几页，那是规则的效果，不是开销。

### M3c 之后复测（2026-09-01，同一台参考机、同一条 `flutter drive --profile`）

| 单章 UTF-16 | 新：冷首屏 | 新：整章补齐 | 新：翻页（帧内） | 新：帧外补邻页 | 新：字号重排到首屏 |
|---|---|---|---|---|---|
| 20 000 | 53 ms（先出 3 页） | 110 ms（53 页） | **2.14 ms/页** | 12.8 ms | 94 ms |
| 50 000 | 27 ms | 148 ms（133 页） | **2.10 ms/页** | 27.4 ms | 120 ms |
| 200 000 | 57 ms | 566 ms（524 页） | **1.91 ms/页** | 27.3 ms | 117 ms |

**翻页那一帧涨了 0.4 ms**（M3b 是 1.72–1.73，这里 1.91–2.14），帧外补邻页涨了 3–5 ms。
两处都是同一件事：两端对齐的行 Paragraph 带一个占位符，建它比建裸行贵一点。
门槛是 build+raster p95 ≤ 8 ms，仍在预算里——但这是 M1 以来**第一次翻页成本涨**，
记在这儿，M3e 的悬挂/压缩要是再往行 Paragraph 上加东西，得先看这一列。

测量那条路一位没动（整章补齐与字号重排都落在 M3b 的噪声里）：两端对齐是绘制键。

### M3d 之后复测（2026-09-01，同一台参考机、同一条 `flutter drive --profile`）

| 单章 UTF-16 | 新：冷首屏 | 新：整章补齐 | 新：翻页（帧内） | 新：帧外补邻页 | 新：字号重排到首屏 |
|---|---|---|---|---|---|
| 20 000 | 55 ms（先出 3 页） | 107 ms（53 页） | 2.04 ms/页 | 13.7 ms | 97 ms |
| 50 000 | 26 ms | 146 ms（133 页） | 2.04 ms/页 | 25.9 ms | 117 ms |
| 200 000 | 60 ms | 569 ms（524 页） | 1.93 ms/页 | 27.1 ms | 126 ms |

**每一档都落在 M3c 的噪声里**。页底对齐是 `compose` 里的一趟后置遍历（纯函数），
绘制那一侧只是多了一次乘加。

#### 那笔「字号重排压线」的账：为什么**不**按原方案做

50k 那档 162 ms（门槛 150 ms），M2c 之后仍然压线。M2a 设想的解法是「装页器支持从
锚点块起装**后缀**（先出锚点页、再往回补页号）」。这一片把它想清楚了，结论是
**按那个方案做出来更差**，所以不做（**M3a 后记**：这笔账已经还了，还它的是
换掉取行区间那条 API，见上一节。下面这段分析仍然成立——它说的是「后缀装页那个
方案不该做」，与账还没还是两件事）：

- **页边界是全局的。** 一页从哪儿起，取决于它前面每一块有多高。所以「锚点那一页」
  的真实边界必须等锚点之前的块全部量完 —— 这不是装配问题，是分页本身的性质；
- 于是后缀装页只有两种收法，两种都比现在差：
  1. **先按「锚点自己成为一页的第一个字」出屏，等前缀补齐后再整体重排**。那 100 ms
     省下来了，但补齐的那一刻页边界会变，读者眼前的正文**当场跳一下**。
     用 100 ms 的等待换一次跳动，在阅读里是亏的；
  2. **让锚点块永久成为一个硬分页点**。不跳了，但那一页之前会留下一页「短页」，
     往回翻就能看见。每改一次字号留一处，而它们不自愈；
- **代价还不止 UX。** 后缀装页要让页表带上「这一份从第几块起」，于是 §5.5 那条
  「`page[0].start == 0`」要放宽 —— 那是这套设计里最硬的一条不变量，
  它挡住过 M2a 的虚拟标题块（会被并页吃掉）和 M2b 的空 range。为一个**暂定**门槛
  的 12 ms 去松它，不划算；
- **门槛本身还没锁。** §10 开头写着「M0 实测后锁定最终数字；实施前暂用以下门槛」，
  上一节也写了「一台参考机、每档一次采样，不够锁 p95」。162 ms 是这台电视盒子
  （作悲观下限）上按了一次滑块的数，手机上会更好。

**所以这笔账的处置是：写清楚原方案更差，把门槛留到有采样时再定，不在这里硬凑。**
真要压这 12 ms 而又不动不变量，唯一干净的路是让锚点**之前**的块测得更快
（例如按几何键复用上一版的行度量 —— 但换字号时它们本来就作废，所以这条只对
「只换了视口高度」有效，而那条 M2c 已经不重测了）。记着，别再重新发明后缀方案。

## 11. 风险与控制

| 风险 | 控制 |
|---|---|
| 误以为 Rust 自动更快 | M0 分段 benchmark；禁止用语言替换代替算法改进 |
| FFI 小对象过多 | typed arrays 批量传输；一次重排固定次数往返 |
| UTF-8/UTF-16 混用 | 强类型 offset + SourceMap + surrogate/emoji property tests |
| 平台字体页数不同 | 进度按正文 offset；缓存键包含 font fingerprint；不持久化 PagePlan |
| Paragraph 跨页复用绘制错误 | clip/translate golden + 首尾行命中测试 |
| 快速改设置产生旧结果回写 | session/generation 校验在 Rust 与 Dart 两侧都执行 |
| 图片让分页停滞 | 超高图片策略和“每步必须推进”不变量 |
| 中文排版把项目拖成 Legado 移植 | 只取算法与语料，不建对照 harness、不移植 UI 状态；每项能力单独里程碑 |
| Paragraph 资源泄漏 | LRU 预算、显式 dispose、退出页生命周期测试 |
| M3 规则推翻 M1 协议 | Placement 第一天就行级；backend 换绘制方式不换协议。**M3a 兑现了一次**：换掉取行区间的 API，页表、线格式、装页器一处没改 |
| 缩进/图片 offset 口径漂移 | §5.1 两条契约 M0 定稿；改口径算破坏性变更，要写进度迁移 |
| 「隐藏标题」开关让全章 offset 平移 | Rubato 不做这个开关（§5.1）；真要做时，补偿是它自己的账 |
| 设置里混进一个几何位却当成绘制位 | 判据 ④ 判「换配色后 `measureCount` 不变、页表是同一个对象」，混进去当场红 |
| 预取放大正文缓存缺陷 | 先补可取消异步取正文与缓存限额，再开 L2 预取 |
| 过早做原生渲染 | M5 决策门；没有 A/B 证据不得启动 |

## 12. 交付顺序

严格按 `M0 → M1 → M2 → M3 → M4` 推进。允许 M2 的视觉设计与 M1 并行准备，
但不得在 M1 的协议和锚点尚未稳定时提前实现 M3 中文算法。M4 的命中测试建立在 §5.4 的
行级 Placement 上；M3 换行来源变化不得改动 M4 的接口。

**这条顺序兑现了**（2026-09-02，M4 收盘）：M4 落地时 §5.4 的行级 `Placement`
一个字段没动——命中、选择、批注、搜索、翻页动画五件事**全部**建在
「某块的第 i..j 行」这一条协议上。M4a 只往控制器上加了一条遍历
（`forEachVisibleLine` / `forEachLineOnPage`），M4e 只给绘制器加了一个页号参数。
M1 那句「后续加入禁则、图片、划线与选择时只换『行从哪来』，不换 `PagePlan` 的形状」
（§2.1 目标 5）到这里第六次兑现。

每个里程碑的提交顺序：

1. 先加 fixture、判据或 benchmark；
2. 再实现最小产品路径；
3. 跑 Rust 单测、Flutter 测试与受影响集成测试；
4. 更新本计划的实测数字与完成状态；
5. 编译/测试结束后终止残留 Flutter、Gradle、Rust 编译与测试进程，避免长期占用内存。

第一件实施工作是 **M0，不是创建 `reader-layout` crate**。没有基线和 schema，暂不改产品代码。

**2026-09-02：M0～M4 全部收盘，并清过一轮账（§8.6）。** 欠着的账仍然**各记在各自那一节
的末尾**（M1 的在 M1 后面，M2a 的在 M2a 后面，以此类推）—— §8.6 只做三件事：
记这一轮还了哪几笔、核出哪几条记错了、把「是决策不是债」那一堆重新标注。
**动手之前先读那一节的分堆**，别照着单条账直接开工：这一轮四十来条里，
三条是记错的，十一条压根不该还。

**两台都跑过了**（§8.6 的表），这一轮没有留空档。下一件事按 §8.6 的分堆走 ——
「有前置」那一堆各归各线（`caches` 淘汰归存储、批注列表屏归界面），排版这条线上
剩下的就只有 M5 那道门了。

## 13. 决策记录

三条要跨里程碑生效、又容易被人重新翻出来问的决策，记在这里；不单开 `docs/adr/` 目录，
三条决策撑不起一套编号体系。

### ADR-001 首版不采用 Rust glyph renderer

**状态**：已定，2026-09-01。推翻条件见 §8 M5。

**背景**：另一条路是 Rust 侧自己做 shaping + 光栅（Parley/swash 等），Flutter 只贴纹理，
能换来跨平台像素一致，也不受 `ui.Paragraph` 的 API 能力限制。

**决策**：首版不做。

**理由**：

- **瓶颈不在 Paragraph，在怎么用它。** M0 实测：逐行取行区间用错 API 差 200 倍
  （真机 648 ms vs 136 406 ms，§5.3），绘制方式选错差 70 倍（0.34 ms vs 23 ms，§5.4）。
  两处都是用法问题，换渲染器一个也解决不了；
- **混合方案的实测离门槛不远。** 参考机上一页绘制 0.34 ms、50k 章测量 245 ms，
  差距靠分批与缓存能补，不需要换渲染栈；
- **Rust 渲染要自带一整条链**：字体发现、系统 fallback（中日韩 + emoji）、shaping、
  光栅或 GPU atlas、DPR 变化、Bidi、选择命中。这些在 Flutter 里是免费的，自己做要
  一条条重新正确；
- **它买到的东西现在没人要**：没有产品需求要求跨平台页码一致（§3 已明确不对齐）。

**后果（接受什么）**：

- 平台间同一章的页数可能不同——所以进度按正文 offset 存，不存页码（§5.5）；
- 受 `ui.Paragraph` 的能力边界限制，典型的就是拿不到 per-glyph ink bounds，见 ADR-002；
- 字体度量不可控，随系统字体与版本变化——缓存键含 font fingerprint（§5.2）。

### ADR-003 Dart 侧不建第二层 Repository，也不按 feature 迁目录

**状态**：已定，2026-09-03。

**背景**：`lib/pages/` 里十三个页面直接 `import ... api/rubato.dart as rust` 调 FFI
（`book_detail_page` 一个文件 27 处、`explore_page` 15、`bookshelf` 11、`home` 10、
`search` 9），页面既是渲染器又是取数器；照 Flutter 官方那套分层建议，很自然会提出两条
改法——**在 Dart 侧建 `BookshelfRepository` / `SourcesRepository`**，以及**把
`lib/pages/` 迁成 `lib/ui/features/<feature>/`**。这两条被提过一次，这里记下为什么不做。

**决策**：两条都不做。真正要解的那两件（重复的派生逻辑、跨页看不见）用别的办法解，
见「改成了什么」。

**理由**：

- **仓储层已经有了，在 Rust 那边。** `docs/data-storage-plan.md` §28 写死「UI 和网络
  不能绕过 Repository 直接拥有数据」，§4.1 那张分层图把 `BookshelfRepository` /
  `SourceRepository` 等六个放在 **Rust Repository / Use Case** 那一层，§82 又说
  「业务事实源在 Rust，Android 与桌面共用同一套」。也就是说 `rust.bookshelf()`
  **本身就是仓储调用**，Dart 侧再包一层一对一转发的同名类，正好撞在同一份文档
  §4.1 末尾那句「不是为了增加空壳类」上；
- **目录迁移拿唯一的守卫换零功能收益。** `integration_test/app_test.dart` 按它自己的
  文件头说，是「产品接线那一层唯一的守卫」，而且为了界面重做 N0 把搜书入口从书架的
  FAB 挪到发现页的 `SearchBar`，整条判据红过一段时间、修的时候发现路线整条换了
  （见 M4b 的账）。迁目录会同时动它的 import 与 `inPage` 限定，换来的只是文件位置。

**改成了什么**（真问题不是分层，是这两件）：

- **派生逻辑重复**：`where((s) => s.enabled).length` 在 `profile_page` /
  `sources_hub_page` / `sources_sheet` 三处逐字相同，`enabled && explorable`
  在 `home_page` / `explore_page` 又各写一遍。收进 `lib/data/live.dart` 的
  `SourceCounts` 扩展，五处共用一份定义；
- **改了别处看不见**：原先靠 `AppShell._visited`（「刚切到第几个 tab」）喊一声、各页
  自己再查一遍——`IndexedStack` 下五个 tab 一直活着不会重建，只能这么补。于是同一份
  `listSources()` 被三个页面各查各的，而且在详情页加了书、在书源面板关了源，别的 tab
  得等你切过去才知道，桌面端分栏的另一栏干脆一直是旧的。换成 `lib/data/live.dart` 的
  `shelfLive` / `sourcesLive`：**一份内存副本 + 失效通知**，写入方改完喊一声
  `refresh()`，所有在看的地方一起变。它不持有事实源、不做写入编排、不落盘——
  写入照旧直接调 `rust.*`；
- **共用副本要防乱序覆盖**：两次 `refresh` 叠在一起是常态（`AppShell` 起来取一次、
  `HomePage` 见 `loading` 没翻会再催一次、切 tab 的兜底可能压在上一次没回来时），
  而 FRB 把调用派到 Rust 工作线程，先发的不保证先回。重复取数无所谓，**跨过一次写入
  就会咬人**：在详情页移出一本书、那次 `refresh` 先回，一次更早发出的重取后回，
  删掉的书会在界面上复活到下一次重取为止。`Live` 内部按代号作废旧结果（规矩同
  `ReaderLayoutController` 那边按 taskId 作废旧任务），判据见
  `test/bookshelf_page_test.dart`「后发的那次说了算」。

**后果（接受什么）**：

- **切 tab 的兜底重取留着，没删干净**。阅读进度是阅读页**防抖落库**的，不经 Dart，
  没有哪个调用点知道它什么时候变；所以 `AppShell._onTab` 仍在进首页/书架时重取一次。
  界面里改的东西已经不靠它了，但这一路还靠；
- **`Live` 是全局的**。选它不是图省事：加书发生在详情页（路由栈深处），要更新的是书架
  那个 tab（兄弟节点），把 notifier 顺着路由栈一层层传下去，正是这类全局通知
  要绕开的管道。代价是判据之间共享状态，靠 `tearDown` 把 `fetch` 还回产品路径；
- **换来了页面级 widget 判据**。`Live.fetch` 可换（同 `BookCover.fetch` /
  `ReaderLayoutController.opener` 的规矩：产品路径是默认，判据把它顶掉），
  于是 `test/bookshelf_page_test.dart` 能在没加载 Rust 动态库的宿主里跑起整页——
  `lib/pages/` 十三个页面此前一条 widget 判据都没有，产品接线那一层全压在
  `app_test.dart` 一条真机判据上。

### ADR-002 标点悬挂的墨水盒从哪来

**状态**：已定，2026-09-01。

**背景**：Legado 的悬挂宽度来自 `Paint.getTextBounds` 的墨水盒（`ZhLayout.kt:213-221`），
而 `ui.Paragraph`/`TextPainter` 只给 advance box，没有 per-glyph ink bounds 的公开 API。

**候选**：

| 方案 | 评价 |
|---|---|
| a. 随包内置正文字体 + 离线预计算 ink 表 | 用户换字体即失效，且撑大包体积 |
| b. Rust 侧读字体文件取 `glyf`/`CFF` bbox | 拿不到 Flutter fallback 之后实际选中的字体文件路径 |
| c. 按字类取固定比例近似 | 最省，但对不上真实字形 |
| **d. 离屏画单字 + 扫像素求墨水盒** | **选它** |

**决策**：走 (d)，结果按 `(字体指纹, 字号, 字符)` 缓存，启动后异步预热常用标点集。

**理由与实测**（探针 ④，真机 HITV205N + 系统字体，fontSize 40）：

| 字符 | advance | ink | 左空白 | 右空白 |
|---|---|---|---|---|
| `，` | 40.0 | [6, 15] | 6 | 25 |
| `。` | 40.0 | [1, 14] | 1 | 26 |
| `（` | 40.0 | [27, 38] | 27 | 2 |
| `）` | 40.0 | [2, 13] | 2 | 27 |
| `中` | 40.0 | [4, 36] | 4 | 4 |

数值可用且区分度足够——句读类右侧空 25/40，前引类左侧空 27/40，正是悬挂与压缩要的量。
方法不需要拿到字体文件，自动跟随 Flutter 的 fallback 结果，这正是 (b) 做不到的。

**后果**：

- 成本 12 字 155 ms（≈13 ms/字），常用标点集约 40 个 → 首次约 0.5 s。**必须异步预热**，
  不得挡首屏；缓存未命中时该字先不悬挂，预热完再重排；
- 同一探针抓到一条会咬人的事实：这台设备上 `“` 的 advance 只有 **15.4**，不是全角——
  中文引号 fallback 到了西文字体。**排版规则不得假设 CJK 标点都是 1 em**，宽度一律实测，
  禁则表按字符判、宽度按度量判，两者分开；
- 若将来 Flutter 开放 per-glyph ink bounds，直接换掉 (d)，缓存层不动。
