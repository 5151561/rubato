# Rubato

精简的网文阅读器**书源引擎**(Rust)。源自 [Legado](https://github.com/gedoor/legado)
的书源生态,但不承诺与上游持续同步。


完整计划与技术选型见 **[docs/plan.md](docs/plan.md)**；阅读正文排版重构见
**[docs/reader-layout-plan.md](docs/reader-layout-plan.md)**；书源引擎与 Legado 的
逐例性能对比见 **[docs/engine-perf.md](docs/engine-perf.md)**（`tools/time_diff.sh` 产出）。

## 仓库结构

| 目录 | 内容 |
|---|---|
| `rust/` | 引擎 cargo workspace(17 个 crate,边界见 plan);`crates/ffi` 是留给前端的 API 边界,暂无下游 |
| `judge/` | 差分裁判:vendor 自 Kotlin 版的书源引擎(`:engine` + `:rhino`),**冻结不开发**;三个 harness(`:harness` / `:jsharness` / `:wvharness`)是我们的 |
| `fixtures/` | 书源语料(1704 源)、去重 JS 片段(683 段)、差分测试页面 |
| `tools/` | jsoup 裁判、差分脚本、语料抽取器 |
| `docs/` | 计划书与设计文档 |

## 常用命令

```bash
# 引擎:构建 / 测试(cargo test 只跑纯函数面的单测,判据在下面的差分套里)
cd rust && cargo build && cargo test

# ── 差分:十八套,全绿(或不超基线)是 Phase 2 的验收前提 ──
tools/all_diff.sh              # 全部跑一遍 + 汇总,CI 调的就是它
tools/all_diff.sh --only=js,xpath --show=10   # 只跑某几套

# 单套(每套的契约在 fixtures/cases/<套名>/README.md,先读它再动手)
tools/syntax_diff.sh          # rule-syntax:RuleAnalyzer
tools/regex_diff.sh           # regex-compat:java.util.regex 方言
tools/json_diff.sh            # json-compat:jayway JSONPath + AnalyzeByJSonPath
tools/html_diff.sh            # html-compat:jsoup 选择器/序列化 + AnalyzeByJSoup
tools/xpath_diff.sh           # xpath-compat:JsoupXpath 2.5.5 + AnalyzeByXPath
tools/url_diff.sh             # rubato-core:java.net.URL / getAbsoluteURL
tools/charset_diff.sh         # net:字符集嗅探
tools/analyze_url_diff.sh     # net:AnalyzeUrl 离线面(option JSON / 编码 / 页码)
tools/fetch_diff.sh           # net:HTTP 录放全链路 + CookieStore
tools/source_diff.sh          # rubato-core:BookSource 实体与规则位
tools/rule_diff.sh            # rule-engine:AnalyzeRule 全链路
tools/js_diff.sh              # js-host:真 Rhino vs rquickjs(裁判是 :jsharness)
tools/pipeline_diff.sh        # pipeline:WebBook 四步
tools/pipeline_corpus_diff.sh # pipeline-corpus:A 层真实书源 × 四步(JS 是确定性桩)
tools/pipeline_corpus_b_diff.sh # pipeline-corpus-b:B 层 × 四步(真 Rhino vs 真 QuickJS)
tools/top100_diff.sh          # top100:自选 100 常用源 × 四步(判据**按源**算)
tools/webview_diff.sh         # webview:BackstageWebView 的策略层(裁判 :wvharness)
tools/explore_diff.sh         # explore:发现页分类列表 exploreKinds()(裁判 :jsharness)
tools/case_diff.sh <目录名>    # 通用比较入口(--show=N 控制展示条数)

# 「还欠多少」:fixtures/diff-baseline.json 记每套允许的 FAIL 上限,
# 只许往下调 —— 全 0 之前也守得住「不新增」。
# 「测到了多少」:三套 pipeline 的 *_diff.sh 会跟着通过率打**命中面**
# (tools/hit_report.py)—— 通过率不看命中面就是骗人:两侧一起空手而归也叫一致。

# 生成物不入库(CI 的 rust job 也跑这一条,几秒钟)
tools/check_generated.sh

# 语料抽取(fixtures/sources/*.json → fixtures/corpus-js/)
python3 tools/extract_js.py

# Phase 3 的分母:哪些源真的用到 webView / 登录 / 验证码(口径 fixtures/phase3/README.md)
tools/gen_phase3_checklist.py
# 逐源验收那条线(tools/phase3_probe.sh)跑在 Flutter 集成测试里,已随前端一并移除。

# Android 交叉编译引擎(单独验证时)
cd rust && ANDROID_NDK_HOME=~/android-sdk/ndk/28.2.13676358 cargo ndk -t arm64-v8a build

# 裁判(唯一还需要 gradle 的地方)
cd judge && ./gradlew :engine:compileDebugKotlin
```

CI 在 `.github/workflows/ci.yml`:`rust` job 跑构建与单测(几分钟),
`diff` job 跑上面那十八套(要 JDK 21 + Android SDK,裁判的 `:engine` 是
android-library 模块)。

## 许可

本仓自己写的部分(`rust/`、`tools/`、`fixtures/` 里的手写用例、`judge/` 下的三个
harness:`:harness` / `:jsharness` / `:wvharness`)按 [MIT](LICENSE) 发布。

**但 `judge/` 里 vendor 来的部分不归 MIT 管,它们保持上游各自的许可:**

| 路径 | 来源 | 上游许可 |
|---|---|---|
| `judge/engine/` | [gedoor/legado](https://github.com/gedoor/legado) 的书源引擎 | GPL-3.0 |
| `judge/rhino/` | 同上(`com.script`,含反编译产物) | GPL-3.0 |
| `judge/third_party/maven/org/htmlunit/htmlunit-core-js/` | 预编译 jar,来源见同目录 `SOURCE.md` | 见上游(Rhino 系,MPL-2.0) |

这些代码只作差分裁判用,**冻结不开发**。`SOURCE.md` 里提到的
`app/src/main/assets/licenses/` 随 2026-09-14 移除前端时一并没了,jar 的
许可与 NOTICE 原文请回上游仓库取。
