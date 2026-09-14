# java-url 差分用例契约

两侧执行器与比较口径同 `fixtures/cases/rule-syntax/README.md`。入口:`tools/url_diff.sh`。

被测:`rust/crates/rubato-core` 的 `java_url` + `net_utils`;裁判侧直接用 JDK 21 的
`java.net.URL`,`NetworkUtils.getAbsoluteURL` 在 harness 垫片里逐行复刻
(原文件带 Android 依赖,挂不进纯 JVM 模块)。

## 用例格式

```jsonc
// NetworkUtils.getAbsoluteURL(URL?, String):base 先按 java.net.URL 解析
{"id":"uh00001","op":"absUrl","base":"http://a.com/x/y.html","path":"../z.html"}
// NetworkUtils.getAbsoluteURL(String?, String):base 取 substringBefore(",")
{"id":"uh00900","op":"absUrlStr","base":"http://a.com/x,{\"method\":\"POST\"}","path":"z"}
```

## 输出

- `{"id":…,"result":"…"}`;`absUrl` 的 base 解析失败 → `{"error":"base_malformed"}`
  (消息不比对)。

## 口径要点(都是差分抓出来的)

- 协议表 = JDK 内置 handler:`http/https/file/ftp/mailto/jrt/jmod/jar`,
  其余(`data:`/`thunder:`/`magnet:`…)抛 MalformedURLException,被 getAbsoluteURL
  吞掉后回落到 **trim 后的原始相对串**;
- JDK 20+ 收紧了 host 合法性:控制字符、空格、DEL 与 `" < > [ \ ] ^ ` { | }`
  非法(userInfo 和 path 不受此限)——书源里 `{{page}}` 落在 host 上就会走这条;
- `parseURL` 里 `spec.indexOf('?')` 是从 **0** 起算的(不是从 start),
  scheme 前缀 + query 的组合要照抄;
- `..` / `.` 归一化只在"相对路径"分支跑,且是 JDK 自己的循环写法,
  与 RFC 3986 的 remove_dot_segments 不等价(`/../../a` 原样保留);
- 传给 `URL(...)` 的是**未 trim** 的原串,只有早退分支用 trim 后的串。

## 已知未覆盖(用例回避,出现即记豁免)

`jar:`(自有 handler,要求 `!/`)、`mailto:`(自有 parseURL)、IPv6 字面量的
畸形写法(Rust 用 `Ipv6Addr` 解析,Java 的 `isIPv6LiteralAddress` 略宽)。
