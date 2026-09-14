// 差分垫片:NetworkUtils / StringExtensions 触到的 android.net 面。
// 差分永远「无网络」(网络面归 net 套与 replay),这里只保证类型齐。
@file:Suppress("unused", "UNUSED_PARAMETER")

package android.net

class Uri private constructor(private val raw: String) {
    val host: String? get() = runCatching { java.net.URI(raw).host }.getOrNull()
    val scheme: String? get() = runCatching { java.net.URI(raw).scheme }.getOrNull()
    val path: String? get() = runCatching { java.net.URI(raw).path }.getOrNull()
    override fun toString(): String = raw

    companion object {
        @JvmStatic
        fun parse(s: String): Uri = Uri(s)

        @JvmStatic
        fun fromFile(f: java.io.File): Uri = Uri("file://" + f.absolutePath)
    }
}

class Network

class NetworkCapabilities {
    fun hasTransport(t: Int): Boolean = false

    companion object {
        const val TRANSPORT_WIFI = 1
        const val TRANSPORT_CELLULAR = 0
        const val TRANSPORT_ETHERNET = 3
        const val TRANSPORT_BLUETOOTH = 2
        const val TRANSPORT_VPN = 4
    }
}

class ConnectivityManager {
    val activeNetwork: Network? = null
    val activeNetworkInfo: NetworkInfo? = null
    fun getNetworkCapabilities(n: Network?): NetworkCapabilities? = null

    companion object {
        const val TYPE_WIFI = 1
        const val TYPE_MOBILE = 0
        const val TYPE_ETHERNET = 9
        const val TYPE_BLUETOOTH = 7
        const val TYPE_VPN = 17
    }
}

class NetworkInfo {
    val isConnected: Boolean = false
    val type: Int = -1
}
