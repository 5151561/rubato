//! 联网探针:逐源跑一次搜索,报告哪些源真的出书。
//!
//!     cargo run --release -p engine --features probes --example probe -- <书源JSON> <关键词> [源数上限]
//!
//! 用途:给 Flutter 的「起步书源包」挑真能用的源,以及粗看 A 层源的联网可用率。
//! 它**不是差分**——真实站点会挂会变,结论只对当次抓取成立。

use engine::Engine;
use engine::shared::CancelToken;
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("用法: probe <书源JSON> <关键词> [上限]");
    let key = args.get(2).cloned().unwrap_or_else(|| "斗破苍穹".into());
    let limit: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);

    let e = Engine::open(":memory:").expect("open");
    let text = std::fs::read_to_string(path).expect("读书源");
    let mut list: Vec<serde_json::Value> = serde_json::from_str(&text).expect("解析书源");
    list.truncate(limit);
    let total = e.import_sources(&serde_json::to_string(&list).expect("序列化")).expect("导入");

    let started = std::time::Instant::now();
    let mut per_source: BTreeMap<String, (String, usize)> = BTreeMap::new();
    let n = e
        .search(&key, &CancelToken::new(), &mut |h| {
            let entry =
                per_source.entry(h.origin.clone()).or_insert_with(|| (h.origin_name.clone(), 0));
            entry.1 += 1;
        })
        .expect("搜索");
    let hits: usize = per_source.values().map(|(_, c)| *c).sum();
    eprintln!(
        "导入 {total} 源 / 跑过 {n} 源 / 出书 {} 源 / 结果 {hits} 条 / 耗时 {:?}",
        per_source.len(),
        started.elapsed()
    );
    // stdout 只出「出书的源」,方便直接喂给挑包脚本
    for (url, (name, count)) in &per_source {
        println!("{count}\t{url}\t{name}");
    }
}
