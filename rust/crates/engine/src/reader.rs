//! 阅读排版的引擎侧装配:layout session、装页入口、正文预取。
//!
//! 计划书 `docs/reader-layout-plan.md` §4.1 / §6.1 / §6.2。
//! 算法在 [`reader_doc`] 与 [`reader_layout`](两个纯函数 crate),
//! 这里只管**状态**:哪一章开着、第几代、谁的结果还算数、预取放不放行。
//!
//! # 取消与「第几代」
//!
//! 计划书 §6.1:「任一新 generation 到来时,旧结果即使完成也不得写入当前状态」。
//! 两侧都判(§11 风险表):
//!
//! - **Dart 侧**:发起时记下自己的代号,Future 落地时对不上就整份丢掉;
//! - **这一侧**:session 记下**见过的最大代号**,比它小的装页请求直接返回
//!   `None` —— 这样即使 Dart 那侧漏判一次,过期结果也进不了产品状态。
//!
//! 代号由 **Dart 生成**(单调递增),不是先跨一次 FFI 领号:领号本身就是一次
//! 往返,而一次重排的往返预算只有两次(计划书 §1)。
//!
//! # 分批测量
//!
//! [`Engine::compose_pages`] 接受只覆盖文档**前缀**的测量批次。装页是流式的,
//! 前缀算出来的页除最后一页外与全量逐位相同([`reader_layout::compose`] 的
//! 用例钉着这条),所以首屏只量到锚点页够用的块就能先画 —— 真机上整章测量是
//! 几百毫秒量级(计划书 §10),先量完整章会把首屏拖到秒级。

use crate::{Engine, EngineError};
use reader_doc::ChapterDocument;
use rubato_core::entities::Book;
use reader_layout::{ComposeParams, DEFAULT_MIN_SPLIT_LINES, MeasureBatch};
use std::sync::atomic::{AtomicI64, Ordering};

/// 同时留着的 layout session 数。当前章 + 前后章 + 一个余量;
/// 超了从最旧的开始丢 —— session 只是「文档 + 代号」,丢了重开就是一次切块。
const MAX_SESSIONS: usize = 4;

/// 同时在飞的正文预取数。**这就是并发上限**(计划书 §6.2):
/// 执行体是 FRB 自己的 worker 池,引擎这一侧只做准入。
const MAX_PREFETCH_INFLIGHT: usize = 2;

/// 阅读设置在通用 kv 里的键。`reader:` 前缀与正文缓存(`content:`)、
/// JS 宿主(`js:`)、设备常量(`env:`)分家
const READER_SETTINGS_KEY: &str = "reader:settings";

static NEXT_SESSION: AtomicI64 = AtomicI64::new(1);

/// 一次「打开某章来排版」的会话
pub struct Session {
    pub id: i64,
    pub book_url: String,
    pub chapter_index: i32,
    pub doc: ChapterDocument,
    /// 见过的最大代号。比它小的装页请求是过期结果
    pub generation: u64,
}

#[derive(Default)]
pub struct SessionRegistry {
    sessions: Vec<Session>,
}

impl SessionRegistry {
    fn insert(&mut self, s: Session) {
        if self.sessions.len() >= MAX_SESSIONS {
            self.sessions.remove(0);
        }
        self.sessions.push(s);
    }

    fn get_mut(&mut self, id: i64) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    fn remove(&mut self, id: i64) {
        self.sessions.retain(|s| s.id != id);
    }
}

/// 打开一章排版会话的产物。文本**只跨一次 FFI**:Dart 拿它建 Paragraph,
/// Rust 这边不留副本(留了就是整章正文在内存里存两份)。
pub struct PreparedChapter {
    pub session_id: i64,
    /// 正文内容指纹。Dart 拿它当整批 Paragraph 缓存的前缀键
    pub revision: u64,
    pub source_utf16_len: u32,
    pub block_count: u32,
    /// 块表,线格式见 [`reader_doc::wire`]
    pub blocks: Vec<u8>,
    pub text: String,
}

impl Engine {
    /// 打开一章:取正文(可取消)→ 接上标题 → 切块 → 建会话。
    ///
    /// 与 [`Engine::chapter_content`] 的区别只有一个:那条是给诊断口用的裸文本,
    /// 这条额外产出文档模型与会话。正文缓存那一步是同一份。
    ///
    /// **标题接进正文**(`reader_doc::ChapterDocument::build_with_title` 那段注释
    /// 写了为什么不是虚拟块)。标题取自目录 —— 目录取不到时按无标题走,
    /// 不为它多报一个错:标题没了顶多少一行,正文还在。
    pub fn open_chapter(
        &self,
        book_url: &str,
        index: i32,
        cancel: &crate::CancelToken,
    ) -> Result<PreparedChapter, EngineError> {
        let text = self.chapter_content_with(book_url, index, cancel)?;
        let title = self
            .stores()
            .books
            .get_chapter(book_url, index)?
            .map(|c| c.title)
            .unwrap_or_default();
        self.open_doc(book_url, index, &title, text)
    }

    /// **判据口**:直接拿一段正文开会话,不经书架、目录与网络。
    ///
    /// M1 的验收要跑在**离线、可复算**的语料上(分页不变量、新旧对照、
    /// 200k 的曲线);挂一本真书既不稳定也复算不了 —— 站点改版就换了语料。
    /// 与 `search_one_page` / `explore_one_page` 同类:诊断口,不是产品路径。
    ///
    /// 不接标题:判据要的是「这一段正文排出什么」,多一个标题块只会让
    /// 那些逐 offset 对照的断言全部要减去一个标题长度。
    pub fn open_text(
        &self,
        book_url: &str,
        index: i32,
        text: String,
    ) -> Result<PreparedChapter, EngineError> {
        self.open_doc(book_url, index, "", text)
    }

    /// 同上,但**显式给标题** —— 判据要验「标题成为第一个块」这条路时用。
    pub fn open_doc_for_judge(
        &self,
        book_url: &str,
        index: i32,
        title: &str,
        text: String,
    ) -> Result<PreparedChapter, EngineError> {
        self.open_doc(book_url, index, title, text)
    }

    fn open_doc(
        &self,
        book_url: &str,
        index: i32,
        title: &str,
        text: String,
    ) -> Result<PreparedChapter, EngineError> {
        let doc = ChapterDocument::build_with_title(format!("{book_url}#{index}"), title, &text)
            .map_err(|e| EngineError::Pipeline(e.to_string()))?;
        let blocks = reader_doc::wire::encode_blocks(&doc);
        let doc_text = reader_doc::document_text(title, &text);
        let prepared = PreparedChapter {
            session_id: NEXT_SESSION.fetch_add(1, Ordering::SeqCst),
            revision: doc.revision,
            source_utf16_len: doc.source_utf16_len,
            block_count: doc.blocks.len() as u32,
            blocks,
            // **文档文本**,不是抓回来的裸正文:接了标题就多一行。
            // Dart 按块表切片,切的必须是同一份
            text: doc_text,
        };
        self.reader_sessions().insert(Session {
            id: prepared.session_id,
            book_url: book_url.to_string(),
            chapter_index: index,
            doc,
            generation: 0,
        });
        Ok(prepared)
    }

    /// 装页。`measures` 是行度量批次(线格式见 [`reader_layout::measure`]),
    /// 可以只覆盖文档前缀。
    ///
    /// `scroll` = 滚动模式:不断页,整章排成一页,`origin_y` 是章内绝对坐标
    /// (计划书 M2c)。**与正文获取无关** —— 换模式不换会话、不重取正文,
    /// 甚至不用重新测量:同一份行度量装两遍就是两种模式。
    ///
    /// 返回 `Ok(None)` = **这次请求已经过期**(有更新的代号来过),
    /// 调用方原样丢掉,不是错误。会话不存在才是 [`EngineError::NotFound`] ——
    /// 那说明调用方拿着已经关掉的会话在用,该当场看见。
    pub fn compose_pages(
        &self,
        session_id: i64,
        generation: u64,
        viewport_height: f32,
        scroll: bool,
        measures: &[u8],
    ) -> Result<Option<Vec<u8>>, EngineError> {
        // 孤行控制按中文排版惯例开着(计划书 M3b)。**不是设置**:
        // 它没有「关掉」这个口径 —— 传 1 只在用例里用,那是为了让
        // 「这条规则到底改了什么」有个对照
        let params = ComposeParams {
            viewport_height,
            scroll,
            min_split_lines: DEFAULT_MIN_SPLIT_LINES,
            // 页底对齐(计划书 M3d)。同样**不是设置**:它只收「差一点点就满」
            // 的那截空白,摊到每行头上是一两个像素 —— 没有「关掉」这个口径
            bottom_align: true,
        };
        let batch =
            MeasureBatch::decode(measures).map_err(|e| EngineError::Pipeline(e.to_string()))?;
        let mut reg = self.reader_sessions();
        let session = reg
            .get_mut(session_id)
            .ok_or_else(|| EngineError::NotFound(format!("layout session {session_id}")))?;
        if generation < session.generation {
            return Ok(None);
        }
        session.generation = generation;
        let out = reader_layout::compose(&session.doc, &batch, params)
            .map_err(|e| EngineError::Pipeline(e.to_string()))?;
        if !out.diagnostics.is_clean() {
            // 计划书 §5.5:不许静默兜底。响一声,别让「正文被切碎」查无线索
            eprintln!(
                "[reader-layout] 装页诊断非空:{}#{} 单行高于整页 {} 次、并页 {} 次",
                session.book_url,
                session.chapter_index,
                out.diagnostics.oversized_lines,
                out.diagnostics.merged_pages
            );
        }
        Ok(Some(reader_layout::encode_pages(&out)))
    }

    /// 关掉会话(退出阅读页 / 换章)。对不存在的号是空操作。
    pub fn close_chapter(&self, session_id: i64) {
        self.reader_sessions().remove(session_id);
    }

    /// **正文预取**:把某一章抓进正文缓存,已缓存或超并发上限就立刻返回 `false`。
    ///
    /// 计划书 §6.2 要的「预取队列与并发上限」在这里是一道**准入闸**:
    /// 执行体是 FRB 自己的 worker 池(Dart 发起一个不 await 的调用),
    /// 引擎只判「这一章要不要抓、现在准不准抓」。自己再起一套线程池,
    /// 换来的只是多一条要管生命周期的线程。
    ///
    /// 只预取**相邻章**是有意为之:`caches` 表至今没有容量与淘汰
    /// (data-storage-plan 的 P2 那条),而相邻章正是用户下一步本来就会抓的,
    /// 所以缓存的增长速度不会因为预取而变快。跨度更大的预取要等那条债还上。
    pub fn prefetch_chapter(
        &self,
        book_url: &str,
        index: i32,
        cancel: &crate::CancelToken,
    ) -> Result<bool, EngineError> {
        if index < 0 || cancel.is_cancelled() {
            return Ok(false);
        }
        let Some(chapter) = self.stores().books.get_chapter(book_url, index)? else {
            return Ok(false);
        };
        if self.stores().books.get_content(book_url, &chapter.url)?.is_some() {
            return Ok(false);
        }
        let key = (book_url.to_string(), index);
        {
            let mut gate = self.prefetch.lock().expect("预取闸锁");
            if gate.len() >= MAX_PREFETCH_INFLIGHT || gate.contains(&key) {
                return Ok(false);
            }
            gate.insert(key.clone());
        }
        let out = self.chapter_content_with(book_url, index, cancel);
        self.prefetch.lock().expect("预取闸锁").remove(&key);
        out.map(|_| true)
    }

    /// 取正文里的一张图片。返回原始字节,**解码在 Dart**(那边才有平台的
    /// 图片解码器与 `ui.Image`)。
    ///
    /// 走这条而不是 Dart 侧 `Image.network`:书源的图片十之八九要带着这个源
    /// 自己的 header(Referer / UA)与 cookie 才取得下来,而那两样都在
    /// `with_env` 这条链路上。绕过去等于把防盗链站点的图片全丢掉。
    ///
    /// `src` 是块表交出来的**原样**那一串(可能带 `,{"width":"50%"}` 的
    /// option JSON)——`AnalyzeUrl` 自己会切,这里不预处理。
    ///
    /// **不落库**:图片字节没有缓存层。`caches` 是 TEXT 的通用 kv、而它的容量与
    /// 淘汰本身还欠着(data-storage-plan 的 P2),给它塞 base64 的图片只会把
    /// 那笔债做大。当前的缓存是 Dart 侧按 URL 记的一份内存表,随章一起丢 ——
    /// 记在计划书 M2b 的账上。
    pub fn chapter_image(
        &self,
        book_url: &str,
        src: &str,
        cancel: &crate::CancelToken,
    ) -> Result<Vec<u8>, EngineError> {
        let row = self.get_book(book_url)?;
        self.book_image(&row.book.clone(), src, cancel)
    }

    /// 同一条取图链路,但**书由调用方给**,不去库里查。
    ///
    /// 分出这一支是因为封面:搜索结果那一屏的书**还没入库**,
    /// [`Self::chapter_image`] 的 `get_book` 必然落空 —— 而封面恰恰是那一屏
    /// 最该带着书源 header / cookie 去取的东西(防盗链站点认 Referer 与 UA)。
    /// 上层用 `from_hit` 把 SearchHit 就地拼成 Book 再交进来,`book` 绑定与
    /// `base_url` 因此仍然是这本书自己的,不是一个空壳。
    pub fn book_image(
        &self,
        book: &Book,
        src: &str,
        cancel: &crate::CancelToken,
    ) -> Result<Vec<u8>, EngineError> {
        let source = self.source_of(&book.origin)?;
        self.with_env(&source, cancel, |env| pipeline::get_image(env, &source, book, src))
    }

    /// 阅读设置(字号、行距、主题……)。**引擎不解释它的内容**:
    /// 存的是 Dart 那侧编出来的一段 JSON,读回去也交给 Dart 解。
    ///
    /// 为什么放这儿而不是在 Dart 侧接一个 `shared_preferences`:库已经在了,
    /// `caches` 表本来就是通用 kv(前缀分家,见 `store::book_store`),
    /// 为一行设置多拉一个平台插件不划算。
    ///
    /// 读不出来 / 存的是旧版本的形状,一律由 Dart 那边退回默认值 ——
    /// 设置读失败不该把阅读页拦住。
    pub fn reader_settings(&self) -> Result<Option<String>, EngineError> {
        Ok(self.stores().books.get_cache(READER_SETTINGS_KEY)?)
    }

    pub fn set_reader_settings(&self, json: &str) -> Result<(), EngineError> {
        self.stores().books.put_cache(READER_SETTINGS_KEY, json)?;
        Ok(())
    }

    fn reader_sessions(&self) -> std::sync::MutexGuard<'_, SessionRegistry> {
        self.reader.lock().expect("layout session 锁")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reader_layout::{BlockMeasure, LineMeasure};

    /// 不抓网:直接拿一段正文建会话,测的是会话与代号那一半
    fn engine() -> Engine {
        Engine::open(":memory:").expect("open")
    }

    fn seed(e: &Engine, text: &str) -> PreparedChapter {
        e.open_text("t", 0, text.to_string()).expect("open_text")
    }

    fn measures(p: &PreparedChapter) -> Vec<u8> {
        MeasureBatch {
            blocks: (0..p.block_count)
                .map(|i| BlockMeasure {
                    block_id: i as u64,
                    space_before: 0.0,
                    lines: vec![LineMeasure {
                        render_start: 0,
                        render_end: 1,
                        top: 0.0,
                        height: 30.0,
                    }],
                })
                .collect(),
        }
        .encode()
    }

    #[test]
    fn 过期的代号装不出页() {
        let e = engine();
        let p = seed(&e, "甲\n乙");
        let m = measures(&p);
        assert!(e.compose_pages(p.session_id, 5, 700.0, false, &m).expect("compose").is_some());
        // 同一代可以再来(分批测量:先前缀后全量)
        assert!(e.compose_pages(p.session_id, 5, 700.0, false, &m).expect("compose").is_some());
        // 更旧的代号:整份丢掉
        assert!(e.compose_pages(p.session_id, 4, 700.0, false, &m).expect("compose").is_none());
        // 更新的代号照常
        assert!(e.compose_pages(p.session_id, 6, 700.0, false, &m).expect("compose").is_some());
    }

    /// 换模式**不换会话、不重新测量**:同一份行度量装两遍就是两种模式
    /// (计划书 M2c:「页面模式独立于正文获取」)
    #[test]
    fn 同一份测量装得出两种模式() {
        let e = engine();
        let p = seed(&e, "甲\n乙\n丙");
        let m = measures(&p);
        let paged = e.compose_pages(p.session_id, 1, 30.0, false, &m).expect("paged").expect("some");
        let scrolled = e.compose_pages(p.session_id, 1, 30.0, true, &m).expect("scroll").expect("some");
        assert_ne!(paged, scrolled, "两种模式该装出不同的页表");
        // 分页:每块 30 高、页高 30 => 三页;滚动:一页
        let page_count = |buf: &[u8]| u32::from_le_bytes(buf[4..8].try_into().expect("4"));
        assert_eq!(page_count(&paged), 3);
        assert_eq!(page_count(&scrolled), 1);
    }

    #[test]
    fn 关掉的会话再用会报错() {
        let e = engine();
        let p = seed(&e, "甲");
        e.close_chapter(p.session_id);
        assert!(matches!(
            e.compose_pages(p.session_id, 1, 700.0, false, &measures(&p)),
            Err(EngineError::NotFound(_))
        ));
        // 对不存在的号再关一次是空操作
        e.close_chapter(p.session_id);
    }

    #[test]
    fn 会话数有上界() {
        let e = engine();
        let ids: Vec<i64> = (0..MAX_SESSIONS + 2).map(|_| seed(&e, "甲").session_id).collect();
        let reg = e.reader_sessions();
        assert_eq!(reg.sessions.len(), MAX_SESSIONS);
        // 挤掉的是最旧的两个
        assert!(!reg.sessions.iter().any(|s| s.id == ids[0] || s.id == ids[1]));
        assert!(reg.sessions.iter().any(|s| s.id == ids[MAX_SESSIONS + 1]));
    }

    #[test]
    fn 阅读设置原样存取() {
        let e = engine();
        assert_eq!(e.reader_settings().expect("read"), None, "没存过就是 None");
        e.set_reader_settings(r#"{"fontSize":20}"#).expect("write");
        assert_eq!(
            e.reader_settings().expect("read").as_deref(),
            Some(r#"{"fontSize":20}"#)
        );
        // 覆盖写,不是追加
        e.set_reader_settings("{}").expect("write");
        assert_eq!(e.reader_settings().expect("read").as_deref(), Some("{}"));
    }

    #[test]
    fn 预取对没有的章是空操作() {
        let e = engine();
        let cancel = crate::CancelToken::new();
        assert!(!e.prefetch_chapter("http://a/1", -1, &cancel).expect("prefetch"));
        assert!(!e.prefetch_chapter("http://a/1", 0, &cancel).expect("prefetch"));
        cancel.cancel();
        assert!(!e.prefetch_chapter("http://a/1", 0, &cancel).expect("prefetch"));
    }

    /// 造一本书 + 一章目录 + 一份正文缓存,不抓网
    fn seed_book(e: &Engine, title: &str, content: &str) {
        use rubato_core::entities::{Book, BookChapter};
        let row = store::BookRow::new(Book {
            book_url: "http://a/1".into(),
            name: "书".into(),
            origin: "http://a".into(),
            ..Default::default()
        });
        let mut st = e.stores();
        st.books.save_book(&row).expect("save book");
        st.books
            .save_chapters(
                "http://a/1",
                &[BookChapter {
                    url: "http://a/c0".into(),
                    title: title.into(),
                    book_url: "http://a/1".into(),
                    index: 0,
                    ..Default::default()
                }],
            )
            .expect("save chapters");
        st.books.put_content("http://a/1", "http://a/c0", content).expect("put");
    }

    /// 产品路径接标题:标题是**正文里的真字符**(第一个块),
    /// 所以 `durChapterPos` 的原点跟着含标题 —— 与裁判同口径,
    /// 理由写在 `reader_doc::ChapterDocument::build_with_title`
    #[test]
    fn 产品路径把标题接成第一个块() {
        use reader_doc::BlockKind;
        let e = engine();
        seed_book(&e, "第一章 开端", "　　甲乙。");
        let p = e.open_chapter("http://a/1", 0, &crate::CancelToken::new()).expect("open");
        assert!(p.text.starts_with("第一章 开端\n"), "实得 {:?}", p.text);
        assert_eq!(p.block_count, 2);
        assert_eq!(p.source_utf16_len, p.text.encode_utf16().count() as u32);
        let reg = e.reader_sessions();
        let doc = &reg.sessions.iter().find(|s| s.id == p.session_id).expect("session").doc;
        assert_eq!(doc.blocks[0].kind, BlockKind::Title);
        assert_eq!(doc.blocks[1].kind, BlockKind::Paragraph);
    }

    /// 判据口**不接标题**:那些逐 offset 对照的断言不该因为多一个标题块而全部要减一截
    #[test]
    fn 判据口不接标题() {
        let e = engine();
        let p = e.open_text("judge://x", 0, "　　甲乙。".into()).expect("open_text");
        assert_eq!(p.text, "　　甲乙。");
        assert_eq!(p.block_count, 1);
    }

    /// 正文里的 `<img …>` 切成图片块,而且 `src` 区间能原样切出 URL ——
    /// Dart 那一侧就是按这一对下标取图的,不自己再写一遍标签解析
    #[test]
    fn 正文里的图片切成图片块() {
        use reader_doc::BlockKind;
        let e = engine();
        let text = "　　甲乙。\n　　<img src=\"http://a/1.jpg\">\n　　丙丁。";
        let p = e.open_text("judge://x", 0, text.into()).expect("open_text");
        assert_eq!(p.block_count, 3, "缩进不该单独成块");
        let reg = e.reader_sessions();
        let doc = &reg.sessions.iter().find(|s| s.id == p.session_id).expect("session").doc;
        let img = &doc.blocks[1];
        assert_eq!(img.kind, BlockKind::Image);
        assert_eq!(img.render_len(), 0, "图片块的 render 文本为空");
        let u: Vec<u16> = p.text.encode_utf16().collect();
        assert_eq!(
            String::from_utf16(&u[img.src_start.get() as usize..img.src_end.get() as usize])
                .expect("utf16"),
            "http://a/1.jpg"
        );
    }

    /// 取图对不存在的书当场报错 —— 那说明调用方拿着别的书的 URL 在取图,该看见
    #[test]
    fn 取图对不存在的书报错() {
        let e = engine();
        assert!(e.chapter_image("http://a/1", "http://a/1.jpg", &crate::CancelToken::new()).is_err());
    }

    /// 已经在缓存里的那一章**不重抓**。这条是预取的第一道闸:
    /// 少了它,每翻一章都会把相邻两章再抓一遍
    #[test]
    fn 已缓存的章不重抓() {
        use rubato_core::entities::{Book, BookChapter};
        let e = engine();
        let row = store::BookRow::new(Book {
            book_url: "http://a/1".into(),
            name: "书".into(),
            origin: "http://a".into(),
            ..Default::default()
        });
        {
            let mut st = e.stores();
            st.books.save_book(&row).expect("save book");
            st.books
                .save_chapters(
                    "http://a/1",
                    &[BookChapter {
                        url: "http://a/c0".into(),
                        title: "第一章".into(),
                        book_url: "http://a/1".into(),
                        index: 0,
                        ..Default::default()
                    }],
                )
                .expect("save chapters");
            st.books.put_content("http://a/1", "http://a/c0", "　　正文").expect("put");
        }
        // 命中缓存:一次网络都不发,直接 false
        assert!(!e.prefetch_chapter("http://a/1", 0, &crate::CancelToken::new()).expect("prefetch"));
    }
}
