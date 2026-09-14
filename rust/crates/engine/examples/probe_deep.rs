//! 联网深探针:每个源取第一条搜索结果,把**四步走完**(详情 → 目录 → 正文),
//! 报告逐源结果。
//!
//!     cargo run --release -p engine --features probes --example probe_deep -- <书源JSON> <关键词>
//!
//! 与 `probe`(只跑搜索)的分工:probe 回答「哪些源出书」,
//! probe_deep 回答「哪些源**读得下去**」。两者都不是差分——
//! 真实站点会挂会改版,结论只对当次抓取成立。
//!
//! 注意:`books` 表按裁判 schema 在 (name, author) 上有唯一索引,
//! 同名同作者的书跨源会互相顶掉(真身的「换源」语义),所以这里
//! 每个源用**独立的内存库**,免得互相干扰统计。

use engine::Engine;
use engine::shared::CancelToken;
use rubato_core::entities::SearchBook;
use std::collections::BTreeMap;
use std::sync::Mutex;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("用法: probe_deep <书源JSON> <关键词>");
    let key = args.get(2).cloned().unwrap_or_else(|| "完美世界".into());
    let text = std::fs::read_to_string(path).expect("读书源");
    let list: Vec<serde_json::Value> = serde_json::from_str(&text).expect("解析书源");

    // 第一步:一次并发搜索,拿到每个源的首条结果
    let searcher = Engine::open(":memory:").expect("open");
    let total =
        searcher.import_sources(&serde_json::to_string(&list).expect("序列化")).expect("导入");
    let mut first: BTreeMap<String, SearchBook> = BTreeMap::new();
    searcher
        .search(&key, &CancelToken::new(), &mut |h| {
            first.entry(h.origin.clone()).or_insert(h);
        })
        .expect("搜索");
    eprintln!("导入 {total} 源;出书 {} 源,开始逐源走四步…", first.len());

    // 第二步:并发跑「加书架 + 取第一章正文」
    let hits: Vec<SearchBook> = first.into_values().collect();
    let by_url: BTreeMap<&str, &serde_json::Value> =
        list.iter().filter_map(|v| Some((v.get("bookSourceUrl")?.as_str()?, v))).collect();
    let queue = Mutex::new(hits.into_iter());
    let out = Mutex::new(Vec::<(bool, String, String, String)>::new());
    std::thread::scope(|s| {
        for _ in 0..6 {
            s.spawn(|| {
                loop {
                    let Some(hit) = queue.lock().expect("队列").next() else { break };
                    let Some(raw) = by_url.get(hit.origin.as_str()) else { continue };
                    let e = Engine::open(":memory:").expect("open");
                    e.import_sources(&raw.to_string()).expect("导入");
                    let line = match e.add_to_bookshelf(&hit) {
                        Ok(row) => match e.chapter_content(&row.book.book_url, 0) {
                            Ok(text) if text.trim().chars().count() >= 50 => (
                                true,
                                hit.origin.clone(),
                                hit.origin_name.clone(),
                                format!(
                                    "{} 章 / 首章 {} 字",
                                    row.book.total_chapter_num,
                                    text.chars().count()
                                ),
                            ),
                            Ok(text) => (
                                false,
                                hit.origin.clone(),
                                hit.origin_name.clone(),
                                format!("首章太短({} 字)", text.chars().count()),
                            ),
                            Err(err) => (
                                false,
                                hit.origin.clone(),
                                hit.origin_name.clone(),
                                format!("正文:{err}"),
                            ),
                        },
                        Err(err) => (
                            false,
                            hit.origin.clone(),
                            hit.origin_name.clone(),
                            format!("目录:{err}"),
                        ),
                    };
                    out.lock().expect("输出").push(line);
                }
            });
        }
    });

    let mut rows = out.into_inner().expect("输出");
    rows.sort();
    let ok = rows.iter().filter(|r| r.0).count();
    eprintln!("走通四步:{ok} / {} 源", rows.len());
    for (ok, url, name, note) in rows {
        println!("{}\t{url}\t{name}\t{note}", if ok { "OK" } else { "FAIL" });
    }
}
