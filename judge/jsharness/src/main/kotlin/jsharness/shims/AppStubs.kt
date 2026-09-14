// 差分垫片:BuildConfig 与 R(字符串资源只在异常消息里出现)
@file:Suppress("unused", "ClassName")

package io.legado.app

object BuildConfig {
    const val DEBUG = false
}

object R {
    object string {
        const val error_get_web_content = 1
        const val chapter_list_empty = 2
        const val replace_rule_invalid = 3
        const val book_no_available_bookSource = 4
        const val content_empty = 5
        const val un_pay_chapter = 6
        const val intro_show_null = 7
        const val intro_show = 8
    }
}
