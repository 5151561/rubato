# source 差分用例契约(BookSource 反序列化)

入口:`tools/source_diff.sh`。被测面 = **书源 JSON → BookSource 实体**的
GSON 语义(pipeline 的入口数据;字段错一个,四步全歪)。

- 裁判:`GSON.fromJson(json, BookSource::class)`,GSON 配置逐字复刻真身
  utils/GsonExtensions.kt(IntJsonDeserializer / StringJsonDeserializer /
  LONG_OR_DOUBLE + 五个 rule 实体的 jsonDeserializer);实体为 shims
  (真身带 Room/Parcelize),字段逐字复刻。
- 被测:`rubato-core::entities::BookSource::parse`(经 core::gson 的 lenient 面)。

## 用例格式 / 输出

```jsonc
{"id": "sc00001", "op": "parseSource", "json": "<书源 JSON 字符串>"}
→ 成功:投影全部流水线相关字段(21 个标量 + 五个 rule 对象或 null)
→ 失败:{"error": "source_error"}
```

## 已踩实的 GSON 语义(两侧一致)

- String 字段:基元一律 asString(数字保**原字面量**、布尔转 "true"/"false"),
  对象/数组转**紧凑 JSON**,null → null;
- Int 字段(自定义适配器):只接受数字(double 截断);字符串数字**不转**,
  落回默认值;
- Long/Boolean(默认适配器):字符串可转("true" 忽略大小写;数字字符串
  parse);类型非法(布尔给数字等)→ **整个源解析失败**;
- rule 对象:接受对象或「字符串包 JSON」(lenient 再解析);数组/其它 → null;
  字符串里不是 JSON 对象 → 整体失败;空串 → null;
- 整体 lenient(单引号/裸键/尾逗号可容);未知键(ruleReview 等被砍面)忽略。

## LegadoTeam 基准下的字段面变动(相对旧 MD3 基准)

- `BookInfoRule` 去掉 `relatedBooks`(旧基准有,新基准无)——投影同步删,
  Rust 侧 `BookInfoRule` 同步删;
- `BookSource` 新增 `mainJs`(JS 单文件书源)。功能本身砍单(docs/plan.md §6),
  但 `isJsSource()` 决定四步流水线的入口走向,故**纳入投影**、两侧都解析;
- `eventListener` / `customButton` / `ruleReview` / `loginUi` 属扩展位与砍单面,
  不进投影。

语料:fixtures/sources 全量(1704 源)逐条 + 类型边角 handmade。
