// 差分垫片:help/source —— 验证码/登录 UI/源级锁。
// (getShareScope / getSharedGlobalStateKey / getSourceType 走真身
//  BaseSourceExtensions.kt,不在这里垫。)
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.source

import io.legado.app.data.entities.BaseSource
import io.legado.app.data.entities.BookSource

object SourceHelp {
    fun openVideoPlayer(source: BaseSource?, url: String, title: String, isFloat: Boolean) = Unit
}

// `SourceVerificationHelp` / `VerificationResult` **换成真身了**(M3j):
// 那 300 行是策略(挂号表、界面互斥、同一个盾只弹一次的飞行表、轮询、取消),
// 与 webView 那 394 行同类 —— 能差分的东西不该留桩。界面那一头由
// `jsharness.VerifyStage` 顶(startActivity 的垫片把 extras 交给它),
// 被测侧的同一份在 `difftest::verify_script`。挂载见 build.gradle.kts。

// `BookSourceExtensions.kt` **整份换成真身了**(M3s):`exploreKinds()` 是发现页
// 分类列表的全部逻辑,而这里此前只有 `getBookType` 的逐字复刻 + `exploreKindsJson`
// 的恒空桩 —— 那等于把要比的那一面挡在门外。挂载见 build.gradle.kts;
// 落盘缓存(ACache)由 shims/ACacheShim.kt 顶成内存表。
