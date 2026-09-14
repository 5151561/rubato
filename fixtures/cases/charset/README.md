# charset 差分用例契约(EncodingDetect / icu4j CharsetDetector)

入口:`tools/charset_diff.sh`(先跑 `tools/gen_charset_cases.py`)。

- 裁判:原样挂载的 `utils/EncodingDetect.kt` + `lib/icu4j/*.java`
  (ICU 3.4 血统的 CharsetDetector,纯 Java,一行未改)。
- 被测:`net::encoding_detect`(meta 判定,经 html-compat)+
  `net::charset_detector`(逐行移植;数据表由 `tools/gen_icu4j_tables.py`
  从冻结源自动抽取到 `net/src/icu4j_data.rs`,勿手改)。

## 用例格式 / 输出

```jsonc
{"id": "cd00001", "op": "detectCharset", "bytesBase64": "...", "tag": "备注,不比对"}
→ {"id": "cd00001", "result": "<charset 名>"}
```

被测函数是 `EncodingDetect.getHtmlEncode(bytes)`:字节窗口找 `<head>…</head>`
(大小写敏感)→ 失败则整串正则(大小写不敏感)→ jsoup 解析 meta 的
charset / http-equiv content-type → 都没有则 icu4j `CharsetDetector.detect()`
→ 无匹配兜底 `"UTF-8"`。

## 刻意保留的上游怪癖(两侧一致即 PASS)

- meta 的 charset **原样返回不校验**(坏名字如 `NoSuchCharset` 原样传出,
  在 fetch 链路里才因 `Charset.forName` 抛错);
- `http-equiv=content-type` 无 `charset=` 时 `substringAfter(";")` 找不到分号
  会把**整个 content 值**(如 `text/html`)当 charset 名返回;
- gb18030 识别器双字节第二位下界是**十进制 80**(0x50,上游笔误);
- 匹配排序:稳定升序后整体 reverse——同分时注册序靠后的识别器赢;
- sbcs/ISO-2022 只看前 8000 字节,UTF-8/16/32 与 mbcs 看全量;
- IBM420/424 识别器默认关闭,未移植。
