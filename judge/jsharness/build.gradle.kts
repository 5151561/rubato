// js-host 差分 harness(纯 JVM):与 :harness 并列的第二个执行器。
//
// 为什么单开一个模块::harness 的 com.script 是**确定性桩**(Phase 1 有意为之,
// 见 harness/shims/ScriptShims.kt),而这一套要的恰恰是**真 Rhino**。同一个
// classpath 上 com.script 只能有一份,故分模块——:harness 那边 92362 例
// 的绿不受影响。
//
// 挂载面:judge/rhino 真身(com.script + htmlunit 补丁类)+ engine 的
// JsExtensions / JsEncodeUtils 真身。Android/UI/webView 依赖由 shims/ 顶掉。
plugins {
    id("org.jetbrains.kotlin.jvm")
    application
}

kotlin {
    jvmToolchain(21)
}

application {
    mainClass = "jsharness.MainKt"
}

// `run` 是 JavaExec:它的环境来自 **Gradle 守护进程**,而守护进程是跨次复用的 ——
// 在 shell 里现设的 RUBATO_DIFF_TIME 不会自己传进来(改 env 不触发守护进程重启),
// 逐例计时就会静默地量不到。所以显式转发这两个变量;走 providers 让它算构建输入。
tasks.named<JavaExec>("run") {
    listOf("RUBATO_DIFF_TIME", "RUBATO_DIFF_ROUNDS").forEach { k ->
        providers.environmentVariable(k).orNull?.let { environment(k, it) }
    }
}

sourceSets {
    main {
        kotlin {
            // 真身 com.script(Rhino 脚本引擎),按原路径挂载
            srcDir(rootProject.file("rhino/src/main/java"))
            srcDir(rootProject.file("engine/src/main/java"))
            include("com/script/**")
            // 真身 java.* 宿主 API(js-host 差分的裁判本体)
            include("io/legado/app/help/JsExtensions.kt")
            include("io/legado/app/help/JsEncodeUtils.kt")
            include("io/legado/app/help/crypto/**")
            // 登录 header / jsLib 的真身入口(Phase 2 判据面之一)
            include("io/legado/app/data/entities/BaseSource.kt")
            include("io/legado/app/data/entities/rule/**")
            include("io/legado/app/utils/GsonExtensions.kt")
            // normalizeJsResult:Rhino 完成值 → 字符串的真身语义(header JS / @js: 都走它)
            include("io/legado/app/model/jsSource/JsSourceEngine.kt")
            include("io/legado/app/data/entities/BookSource.kt")
            include("io/legado/app/data/entities/BookSourcePart.kt")
            include("io/legado/app/constant/BookType.kt")
            include("io/legado/app/constant/BookSourceType.kt")
            include("io/legado/app/constant/SourceType.kt")
            include("io/legado/app/utils/MapExtensions.kt")
            include("io/legado/app/help/source/BaseSourceExtensions.kt")
            // **发现页分类列表的真身**(M3s):`exploreKinds()` 把 `exploreUrl`
            // 摊成分类格子 —— 三条路(JSON 数组 / `title::url` 行 / `@js:`+`<js>`)
            // 全在这一个函数里。此前这里是 shims/LegadoSourceShims.kt 的两个桩
            // (`getBookType` 逐字复刻 + `exploreKindsJson` 恒空),整块面没人比。
            // 落盘那一层(`ACache`)换成内存垫片:见 shims/ACacheShim.kt。
            include("io/legado/app/help/source/BookSourceExtensions.kt")
            // 「请用户出手」那三件的**策略层真身**(M3j:此前是 shims 里那个抛
            // 「未接」的桩)。界面那一头换成剧本用户 —— jsharness/VerifyStage.kt
            // 顶掉 `startActivity`,契约见 fixtures/cases/js-host/README.md
            include("io/legado/app/help/source/SourceVerificationHelp.kt")
            // AnalyzeRule 真身:js 里的 `java` 绑定就是它(java.put/get/getString/getElements)
            include("io/legado/app/model/analyzeRule/RuleAnalyzer.kt")
            include("io/legado/app/model/analyzeRule/AnalyzeByJSoup.kt")
            include("io/legado/app/model/analyzeRule/AnalyzeByJSonPath.kt")
            include("io/legado/app/model/analyzeRule/AnalyzeByXPath.kt")
            include("io/legado/app/model/analyzeRule/AnalyzeByRegex.kt")
            include("io/legado/app/model/analyzeRule/RuleDataInterface.kt")
            include("io/legado/app/model/analyzeRule/RuleData.kt")
            include("io/legado/app/model/analyzeRule/AnalyzeRule.kt")
            // AnalyzeUrl 真身 + 网络面(M2c):`java.ajax`/`connect` 走它,
            // 底下的 okhttp 调用由 shims 的 newCall* 转发到 jsharness/ReplayHttp
            // (HTTP 录放,契约 docs/http-snapshot.md)。此前这里是垫片。
            include("io/legado/app/model/analyzeRule/AnalyzeUrl.kt")
            include("io/legado/app/model/analyzeRule/AnalyzeUrlNetworkOptions.kt")
            include("io/legado/app/help/http/RequestMethod.kt")
            // webView:**策略层真身**挂载(此前是 shims/LegadoHttpShims.kt 里那个
            // 抛「未接」标记的桩)。平台那一半换成剧本 —— 剧本 WebView 在
            // shims/AndroidWebkitShims.kt,时钟在 jsharness/WebViewStage.kt,
            // 契约 fixtures/cases/webview/README.md(与 :harness / :wvharness 同一份)。
            include("io/legado/app/help/http/BackstageWebView.kt")
            include("io/legado/app/help/webView/WebViewRequestConfig.kt")
            include("io/legado/app/help/http/StrResponse.kt")
            include("io/legado/app/help/http/CookieStore.kt")
            include("io/legado/app/help/http/CookieManager.kt")
            include("io/legado/app/help/http/api/CookieManagerInterface.kt")
            include("io/legado/app/utils/CookieManagerExtensions.kt")
            include("io/legado/app/data/entities/BaseBook.kt")
            include("io/legado/app/data/entities/Book.kt")
            include("io/legado/app/data/entities/BookChapter.kt")
            include("io/legado/app/data/entities/SearchBook.kt")
            include("io/legado/app/utils/StringUtils.kt")
            include("io/legado/app/utils/InfoMap.kt")
            include("io/legado/app/data/entities/Bookmark.kt")
            include("io/legado/app/data/entities/ReplaceRule.kt")
            include("io/legado/app/data/entities/ReplaceBook.kt")
            include("io/legado/app/constant/PageAnim.kt")
            include("io/legado/app/exception/RegexTimeoutException.kt")
            include("io/legado/app/exception/TocEmptyException.kt")
            include("io/legado/app/exception/ContentEmptyException.kt")
            // Book/BookChapter 真身触到的正文处理 + WebBook 四步(与 :harness 同挂载面)
            include("io/legado/app/help/book/ContentProcessor.kt")
            include("io/legado/app/help/book/BookContent.kt")
            include("io/legado/app/help/book/ContentHelp.kt")
            include("io/legado/app/utils/HtmlFormatter.kt")
            // WebBook 四步真身(pipeline-corpus-b:B 层书源 × 四步,JS 是真 Rhino)。
            // :harness 那边挂的是同一批文件,区别只在 com.script:那边是确定性桩,
            // 这边是真 Rhino —— 于是 `<js>` / `@js:` 的书源才测得了。
            include("io/legado/app/model/webBook/BookList.kt")
            include("io/legado/app/model/webBook/BookInfo.kt")
            include("io/legado/app/model/webBook/BookChapterList.kt")
            include("io/legado/app/model/webBook/BookContent.kt")
            include("io/legado/app/model/webBook/WebBook.kt")
            include("io/legado/app/data/entities/rule/**")
            include("io/legado/app/utils/InfoMap.kt")

            // 上面两个直接引用的纯 JVM 工具
            include("io/legado/app/utils/MD5Utils.kt")
            include("io/legado/app/utils/StringUtils.kt")
            include("io/legado/app/utils/HtmlFormatter.kt")
            include("io/legado/app/utils/JsURL.kt")
            include("io/legado/app/utils/EncoderUtils.kt")
            include("io/legado/app/utils/EncodingDetect.kt")
            include("io/legado/app/utils/ByteArrayExtensions.kt")
            include("io/legado/app/utils/StringExtensions.kt")
            include("io/legado/app/utils/NetworkUtils.kt")
            include("io/legado/app/utils/ThrowableExtensions.kt")
            include("io/legado/app/utils/Utf8BomUtils.kt")
            include("io/legado/app/constant/AppPattern.kt")
            include("io/legado/app/exception/NoStackTraceException.kt")
            include("jsharness/**")
            include("android/**")
            include("androidx/**")
        }
        java {
            // judge/rhino 里的 htmlunit 补丁类(ConcurrentNativeObject / xmlimpl)
            srcDir(rootProject.file("rhino/src/main/java"))
            include("org/htmlunit/**")
            // EncodingDetect 的兜底:icu4j CharsetDetector(纯 Java,原样挂载)
            srcDir(rootProject.file("engine/src/main/java"))
            include("io/legado/app/lib/icu4j/**")
        }
    }
}

dependencies {
    implementation(libs.gson)
    implementation(libs.kotlinx.coroutines.core)
    implementation(libs.htmlunit.core.js)
    // RhinoClassShutter 的保护类名单直接引用 okio 类型(纯 JVM 库,不必垫片)
    implementation(libs.okhttp)
    // JsEncodeUtils / MD5Utils / EncoderUtils 直接用 hutool(纯 JVM)
    implementation(libs.hutool.core)
    implementation(libs.hutool.crypto)
    // EncodingDetect / JsExtensions 的 Jsoup.parse
    implementation(libs.jsoup)
    implementation(libs.commons.text)
    // AnalyzeByJSonPath / AnalyzeByXPath 的真身依赖
    implementation(libs.json.path)
    implementation(libs.jsoupxpath)
}
