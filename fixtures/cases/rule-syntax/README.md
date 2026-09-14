# rule-syntax 差分用例契约

两侧执行器:
- 裁判:`judge/harness`(纯 JVM 模块,源码直接挂 `judge/engine` 的 RuleAnalyzer.kt,不复制不修改)
- 被测:`rust/crates/difftest` 的 `syntax_case_runner`

入口:`tools/syntax_diff.sh`。

## 用例格式(输入,JSON 数组)

每个元素为一个 case,按 `op` 分三种:

```jsonc
{"id":"h001","op":"split","data":"a&&b","code":false,"trim":false,"sep":["&&","||","%%"]}
{"id":"h050","op":"innerRule","data":"x{$.a}y","inner":"{$.","startStep":1,"endStep":1}
{"id":"h060","op":"innerRuleStr","data":"u/{{p}}","startStr":"{{","endStr":"}}"}
```

- `split`:构造 `RuleAnalyzer(data, code)`;`trim=true` 时先调 `trim()`;然后
  `splitRule(*sep)`。输出 `result`(字符串数组)与 `elementsType`。
- `innerRule`:`RuleAnalyzer(data, false)` + `innerRule(inner, startStep, endStep, fr)`。
  输出 `result`(字符串)。
- `innerRuleStr`:`RuleAnalyzer(data, false)` + `innerRule(startStr, endStr, fr)`。
  输出 `result`(字符串)。

## fr 回调契约(两侧必须一致实现)

内嵌规则解析在真实引擎里回调 JS/规则引擎;差分切分器本身时用固定桩:

- 参数 s 含子串 `NULL` → 返回 null
- 参数 s 含子串 `EMPTY` → 返回 ""
- 否则 → 返回 `«` + s + `»`

## 输出格式(JSONL,每行一个 JSON 对象)

成功:`{"id":"...","result":[...] 或 "...","elementsType":"..."}`(elementsType 仅 split)
失败:`{"id":"...","error":"<消息>"}`

错误消息规范化:
- Kotlin `StringIndexOutOfBoundsException` / Rust `SplitError::IndexOutOfBounds`
  → 统一写 `index_out_of_bounds`
- Kotlin `Error(msg)`(splitRule 的"...后未平衡")/ Rust `SplitError::Unbalanced(msg)`
  → 原样写 msg(两侧逐字对齐)
- 其他异常 → `exception:<类名>:<消息>`(出现即视为待调查项)

比较口径:按 id 逐 case 解析 JSON 后做结构化相等比较(与键序无关)。
diff 不一致默认裁判为准(docs/plan.md §3)。
