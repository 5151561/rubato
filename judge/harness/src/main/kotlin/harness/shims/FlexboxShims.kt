// 差分垫片:com.google.android.flexbox 只被 FlexChildStyle.apply(View) 触到,
// 差分永不调用(发现页 UI)。
@file:Suppress("unused", "PackageDirectoryMismatch")

package com.google.android.flexbox

class FlexboxLayout {
    class LayoutParams {
        var flexGrow: Float = 0F
        var flexShrink: Float = 1F
        var alignSelf: Int = -1
        var flexBasisPercent: Float = -1F
        var isWrapBefore: Boolean = false
    }
}
