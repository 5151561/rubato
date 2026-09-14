//! 差分侧的共享桩。

pub mod js_net;
pub mod pipeline_case;
pub mod replay;
pub mod shared_io;
pub mod stub_host;
pub mod verify_script;
pub mod wv_script;

/// 三个 runner 共用的壳:**先**钉差分 UA → 校验参数 → 读 cases.json →
/// 逐 case 写 jsonl。`f` 收 `(case, http快照根目录)`,不吃快照的套忽略
/// 第二个参数即可。
///
/// **差分的 UA 必须钉死,而且要与裁判垫片同值**:UA 进 HTTP 快照的 key
/// (docs/http-snapshot.md §3),裁判侧 AppConfig 垫片给的是 "rubato-judge"
/// (judge/{harness,jsharness}/…/shims)。产品的默认 UA 是另一个值
/// (net::client 的 PRODUCT_UA)—— 这里第一句就是两者的分界,
/// 必须走在任何 case 之前。
pub fn run_jsonl(
    usage: &str,
    mut f: impl FnMut(&serde_json::Value, Option<&std::path::PathBuf>) -> serde_json::Value,
) {
    use std::io::Write;
    net::client::set_user_agent("rubato-judge");
    let args: Vec<String> = std::env::args().collect();
    if !(args.len() == 3 || args.len() == 4) {
        eprintln!("用法: {usage}");
        std::process::exit(2);
    }
    let snapshot_root: Option<std::path::PathBuf> = args.get(3).map(Into::into);
    let text = std::fs::read_to_string(&args[1]).expect("读取用例文件失败");
    let cases: Vec<serde_json::Value> = serde_json::from_str(&text).expect("用例 JSON 解析失败");
    let mut out = std::io::BufWriter::new(std::fs::File::create(&args[2]).expect("创建输出失败"));

    // 逐例计时(默认关):`RUBATO_DIFF_TIME=1` 打开,给每条输出补一个 `__ns`。
    // 关的时候这条路径与老写法逐字节等价 —— 计时不能改判据面。
    let rounds = timing_rounds();
    if rounds == 0 {
        for c in &cases {
            writeln!(out, "{}", f(c, snapshot_root.as_ref())).unwrap();
        }
        return;
    }
    // 多轮取**每例最小值**:裁判侧是 JVM,第一轮跑在解释器里,不预热出来的
    // 数字是「JIT 没热」而不是「Kotlin 慢」。两侧同一条规则,免得口径分岔。
    // 输出取**最后一轮**,不是各轮拼接 —— 前面几轮只为计时。
    let mut best = vec![u64::MAX; cases.len()];
    for round in 0..rounds {
        let last = round + 1 == rounds;
        for (i, c) in cases.iter().enumerate() {
            let t0 = std::time::Instant::now();
            let mut v = f(c, snapshot_root.as_ref());
            let ns = t0.elapsed().as_nanos().min(u64::MAX as u128) as u64;
            best[i] = best[i].min(ns);
            if last {
                if let Some(o) = v.as_object_mut() {
                    o.insert("__ns".into(), best[i].into());
                }
                writeln!(out, "{v}").unwrap();
            }
        }
    }
}

/// 计时轮数:`RUBATO_DIFF_TIME=1` 打开(否则 0 = 关),轮数由
/// `RUBATO_DIFF_ROUNDS` 给,缺省 3。
pub fn timing_rounds() -> usize {
    if std::env::var("RUBATO_DIFF_TIME").as_deref() != Ok("1") {
        return 0;
    }
    std::env::var("RUBATO_DIFF_ROUNDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(3)
}
