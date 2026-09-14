@file:Suppress("unused", "UNUSED_PARAMETER")

package android.text

// StringExtensions 的 Editable 扩展(差分不用,只为类型齐)
interface Editable : CharSequence {
    interface Factory {
        fun newEditable(source: CharSequence): Editable

        companion object {
            @JvmStatic
            fun getInstance(): Factory = throw UnsupportedOperationException("差分桩")
        }
    }
}
