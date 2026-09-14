// 差分垫片:JsExtensions 的 UI 面(openUrl / showBrowser / toast)。
// 按 plan §6「UI 型 JS API 空实现记日志」处理,只保证类型齐。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.ui.association

class OnLineImportActivity

class OpenUrlConfirmActivity

class VerificationCodeActivity

// 真身 `SourceVerificationHelp` 拉起来的那两个界面。差分里它们只当**类型**用
// —— `startActivity<T>` 的垫片把 extras 交给 `jsharness.VerifyStage`,
// 由剧本用户替人答一句(契约见 fixtures/cases/js-host/README.md 的 `verify`)

