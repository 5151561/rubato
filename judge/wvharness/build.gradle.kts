// webView 策略差分 harness(纯 JVM):与 :harness / :jsharness 并列的第三个执行器。
//
// 为什么单开一个模块:那两个模块里的 `BackstageWebView` 都是**确定性桩**
// (harness/shims/HttpShims.kt、jsharness/shims/LegadoHttpShims.kt —— 回传
// javaScript 原文,fetch / analyze-url / rule-engine / js-host 四套的判据都建在
// 那个契约上)。这一套要的恰恰是**真身那 394 行**,同一个 classpath 上
// `io.legado.app.help.http.BackstageWebView` 只能有一份,故分模块 ——
// 那两套的绿不受影响。
//
// 挂载面:BackstageWebView 真身 + StrResponse 真身(`url()` 的取法是观察面)
// + WebViewRequestConfig 真身(UA 与请求头的取法本身就是判据面)。
// `CookieStore` 只做**录调用**的垫片(理由见 shims/LegadoHttpShims.kt)。
// Android(webkit/os/net.http)、WebView 池、Coroutine 包装由 shims/ 顶掉;
// **真 WebView 换成一份剧本 + 虚拟时钟**,契约见 fixtures/cases/webview/README.md。
plugins {
    id("org.jetbrains.kotlin.jvm")
    application
}

kotlin {
    jvmToolchain(21)
}

application {
    mainClass = "wvharness.MainKt"
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
            // 本套的判据本体
            include("io/legado/app/help/http/BackstageWebView.kt")
            // UA 选取与请求头过滤是**可观察的**,挂真身而不是抄一份
            include("io/legado/app/help/webView/WebViewRequestConfig.kt")
            // `url()` 走 networkResponse / 重定向那一支包 priorResponse,是观察面
            include("io/legado/app/help/http/StrResponse.kt")
            // 「js执行超时」那条报错的类型
            include("io/legado/app/exception/NoStackTraceException.kt")
            include("wvharness/**")
        }
    }
}

dependencies {
    implementation(libs.gson)
    // handleResult 的 StringEscapeUtils.unescapeJson —— 真身直接用它
    implementation(libs.commons.text)
    implementation(libs.kotlinx.coroutines.core)
    // StrResponse / buildStrResponse 直接用 okhttp 的类型
    implementation(libs.okhttp)
}
