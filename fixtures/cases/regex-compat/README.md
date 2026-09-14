# regex-compat 差分用例契约

两侧执行器与比较口径同 `fixtures/cases/rule-syntax/README.md`。
入口:`tools/regex_diff.sh`。

## 用例格式

```jsonc
// AnalyzeRule.replaceRegex 复合语义(##pattern##replacement###)
{"id":"x001","op":"replaceRegex","data":"输入文本","pattern":"\\s+","replacement":"-","first":false}
// 原始匹配语义:所有匹配的分组值(组0..N,未匹配组为 null),上限 50 个匹配
{"id":"x002","op":"regexFind","data":"输入文本","pattern":"(a)(b)?"}
```

## 输出

- `replaceRegex` 永不报错(降级链条在语义内):`{"id":...,"result":"..."}`
- `regexFind`:`{"result":[[组0,组1,...],...]}`;编译失败 → `{"error":"compile_error"}`
  (两侧消息不比对,只比对"是否编译失败");匹配期失败 → Rust `runtime_error`
  / 裁判超时 `timeout`(两者理论上对应同类灾难性回溯,出现即人工检查)

## 已知近似(diff 观测口径)

- Java `(?i)` 默认 ASCII 大小写折叠,Rust 用 Unicode 折叠近似;
- 空匹配推进:Java 按 UTF-16 code unit,Rust 按 char(增补平面字符有差);
- `\G`、`\p{InXxx}`、`(?U)`、类内 `\Q` 未实现(Rust 侧按编译失败降级)。
