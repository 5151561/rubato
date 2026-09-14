// 差分 harness:把引擎里零依赖的 Kotlin 文件按原路径挂进纯 JVM 模块编译,
// 逐字节使用裁判源码(不复制、不修改),对外提供命令行差分执行器。
plugins {
    // 版本随其它模块的 Kotlin 插件走(已在 classpath,不能重复声明版本)
    id("org.jetbrains.kotlin.jvm")
    application
}

kotlin {
    jvmToolchain(21)
}

application {
    mainClass = "harness.MainKt"
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
            srcDir(rootProject.file("engine/src/main/java"))
            // 只挑无 Android/三方依赖的引擎文件;include 作用于全部 srcDir,
            // 故 harness 自身代码放在 harness/ 包下与之匹配
            include("io/legado/app/model/analyzeRule/RuleAnalyzer.kt")
            include("io/legado/app/model/analyzeRule/AnalyzeByJSoup.kt")
            include("io/legado/app/model/analyzeRule/AnalyzeByJSonPath.kt")
            include("io/legado/app/model/analyzeRule/AnalyzeByXPath.kt")
            include("io/legado/app/model/analyzeRule/AnalyzeByRegex.kt")
            include("io/legado/app/model/analyzeRule/RuleDataInterface.kt")
            include("io/legado/app/model/analyzeRule/RuleData.kt")
            include("io/legado/app/model/analyzeRule/AnalyzeRule.kt")
            include("io/legado/app/model/analyzeRule/AnalyzeUrl.kt")
            include("io/legado/app/help/http/RequestMethod.kt")
            include("io/legado/app/constant/AppPattern.kt")
            // webView:**策略层真身**挂载(此前是 shims/HttpShims.kt 里那个
            // 「回传 javaScript 原文」的 Phase 1 桩)。平台那一半换成剧本 ——
            // 剧本 WebView 在 shims/WebkitShims.kt,时钟在 harness/WebViewStage.kt,
            // 契约 fixtures/cases/webview/README.md(与 :wvharness 同一份)。
            include("io/legado/app/help/http/BackstageWebView.kt")
            // UA 选取与请求头过滤是**可观察的**(fetch 那套比 headers),挂真身
            include("io/legado/app/help/webView/WebViewRequestConfig.kt")
            // HTTP 录放 + cookie:真身挂载(Android/Room/webkit 依赖由 shims 顶掉)
            include("io/legado/app/help/http/StrResponse.kt")
            include("io/legado/app/help/http/CookieStore.kt")
            include("io/legado/app/help/http/CookieManager.kt")
            include("io/legado/app/help/http/api/CookieManagerInterface.kt")
            include("io/legado/app/utils/EncodingDetect.kt")
            include("io/legado/app/utils/ByteArrayExtensions.kt")
            include("io/legado/app/utils/Utf8BomUtils.kt")
            include("io/legado/app/utils/CookieManagerExtensions.kt")
            // pipeline(WebBook 四步):实体与四步真身挂载
            include("io/legado/app/constant/BookType.kt")
            include("io/legado/app/constant/BookSourceType.kt")
            include("io/legado/app/constant/SourceType.kt")
            include("io/legado/app/constant/PageAnim.kt")
            include("io/legado/app/data/entities/BaseBook.kt")
            include("io/legado/app/data/entities/BookSource.kt")
            include("io/legado/app/data/entities/ReplaceBook.kt")
            include("io/legado/app/data/entities/rule/**")
            include("io/legado/app/model/analyzeRule/AnalyzeUrlNetworkOptions.kt")
            include("io/legado/app/utils/MapExtensions.kt")
            include("io/legado/app/utils/InfoMap.kt")
            include("io/legado/app/data/entities/Book.kt")
            include("io/legado/app/data/entities/BookChapter.kt")
            include("io/legado/app/data/entities/SearchBook.kt")
            include("io/legado/app/data/entities/BookSourcePart.kt")
            include("io/legado/app/data/entities/Bookmark.kt")
            include("io/legado/app/data/entities/ReplaceRule.kt")
            include("io/legado/app/exception/NoStackTraceException.kt")
            include("io/legado/app/exception/RegexTimeoutException.kt")
            include("io/legado/app/utils/MD5Utils.kt")
            include("io/legado/app/exception/TocEmptyException.kt")
            include("io/legado/app/exception/ContentEmptyException.kt")
            include("io/legado/app/help/book/ContentProcessor.kt")
            include("io/legado/app/help/book/BookContent.kt")
            include("io/legado/app/help/book/ContentHelp.kt")
            include("io/legado/app/utils/StringUtils.kt")
            include("io/legado/app/utils/HtmlFormatter.kt")
            include("io/legado/app/model/webBook/BookList.kt")
            include("io/legado/app/model/webBook/BookInfo.kt")
            include("io/legado/app/model/webBook/BookChapterList.kt")
            include("io/legado/app/model/webBook/BookContent.kt")
            include("io/legado/app/model/webBook/WebBook.kt")
            include("harness/**")
            include("androidx/**")
        }
        java {
            // EncodingDetect 的兜底:icu4j CharsetDetector(纯 Java,原样挂载)
            srcDir(rootProject.file("engine/src/main/java"))
            include("io/legado/app/lib/icu4j/**")
        }
    }
}

dependencies {
    implementation(libs.gson)
    // json-compat 差分:与引擎同版本的 jayway JsonPath(版本目录统一管理)
    implementation(libs.json.path)
    // html-compat 差分:与引擎同版本的 jsoup;AnalyzeByJSoup 引用 JXNode
    implementation(libs.jsoup)
    implementation(libs.jsoupxpath)
    // AnalyzeRule 的直接依赖:实体反转义、协程、Rhino 类型(NativeObject/Scriptable)
    implementation(libs.commons.text)
    implementation(libs.kotlinx.coroutines.core)
    implementation(libs.htmlunit.core.js)
    // AnalyzeUrl 直接用 okhttp 的类型(纯 JVM 库,不必垫片)
    implementation(libs.okhttp)
    // AnalyzeUrl/MD5Utils/StringExtensions 直接用 hutool(纯 JVM)
    implementation(libs.hutool.core)
    implementation(libs.hutool.crypto)
}
