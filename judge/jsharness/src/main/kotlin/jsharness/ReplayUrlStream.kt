// 把 JVM 的 `URL.openConnection()` 整条 http/https 路径接到 HTTP 录放上。
//
// **为什么要有这个**:`JsExtensions.get/head/post`(书源里的 `java.get(url,headers)`)
// 走的不是 okhttp,是 `Jsoup.connect(url)….execute()`,而 jsoup 1.16.2 底下用的是
// `java.net.HttpURLConnection`。这条路 :jsharness 之前**没有拦** —— 差分跑起来会
// 真的去连外网,拿到的还是当天的线上响应。实测 `js-corpus-ea97f51e04` 就把
// `https://www.35xss.com/` 的实时 302 Location 当成了裁判结论:那不是判据,是噪声。
//
// 拦法:装一个全局 `URLStreamHandlerFactory`,http/https 都交给这里,再转
// [ReplayHttp.executeRaw](与 okhttp 那条路共用同一份快照与 key 算法)。
// 于是**整个裁判进程再也发不出真实请求** —— 这既是可重现性,也是判据的前提。
//
// okhttp 不受影响:它走裸 socket,不经过 URLStreamHandler(它那条路本来就已经
// 被 shims 的 newCall* 转到 ReplayHttp 了)。
package jsharness

import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLConnection
import java.net.URLStreamHandler
import java.net.URLStreamHandlerFactory

object ReplayUrlStream {
    private var installed = false

    /** 幂等;`URL.setURLStreamHandlerFactory` 一个 JVM 只允许设一次 */
    @Synchronized
    fun install() {
        if (installed) return
        installed = true
        URL.setURLStreamHandlerFactory(Factory)
        blockRawSockets()
    }

    // **okhttp 那条路的最后一个洞**:书源可以用 LiveConnect 绕开宿主,
    // `new Packages.okhttp3.OkHttpClient()` 自己建一个客户端(语料里 2 个源这么写)。
    // 那是真身的 okhttp 类,不经 shims 的 newCall*,也不经 URLStreamHandler ——
    // 于是裁判会真的去连外网。实测那两例的裁判结论是 **timeout**(单 case 5 秒),
    // 也就是「当天那台主机连不连得上」,不是语义。
    //
    // 拦法:把默认 ProxySelector 换成「一律走 127.0.0.1:1」。那个端口没人听,
    // 连接**立刻**被拒(ConnectException),书源自己的 try/catch 照常吞掉 ——
    // 与被测侧(没有 okhttp3 这个类,TypeError 后同样被吞)收敛到同一处。
    // 这不是伪造语义,是把裁判放进一个「什么都连不上」的网络里,而这正是
    // 「一个字节都不许出网」那条前提本来就要求的。
    private fun blockRawSockets() {
        val dead = java.net.Proxy(
            java.net.Proxy.Type.HTTP,
            java.net.InetSocketAddress.createUnresolved("127.0.0.1", 1)
        )
        java.net.ProxySelector.setDefault(object : java.net.ProxySelector() {
            override fun select(uri: java.net.URI?): MutableList<java.net.Proxy> =
                mutableListOf(dead)

            override fun connectFailed(uri: java.net.URI?, sa: java.net.SocketAddress?, ioe: IOException?) = Unit
        })
    }

    private object Factory : URLStreamHandlerFactory {
        override fun createURLStreamHandler(protocol: String): URLStreamHandler? =
            when (protocol) {
                "http" -> httpHandler
                "https" -> httpsHandler
                else -> null // 其余协议(file/jar/…)交回 JDK 的缺省实现
            }
    }

    // 每个协议一个实例:`getDefaultPort` 是按 handler 问的,拿不到 URL。
    // 给错了 URL.toString 会把 `:80`/`:443` 带出来,进而改掉快照 key
    // (契约 docs/http-snapshot.md §3 要求默认端口省略)。
    private val httpHandler = Handler(80)
    private val httpsHandler = Handler(443)

    private class Handler(private val defaultPort: Int) : URLStreamHandler() {
        override fun openConnection(u: URL): URLConnection = ReplayConnection(u)
        override fun getDefaultPort(): Int = defaultPort
    }

    /**
     * 极简 `HttpURLConnection`:只实现 jsoup 1.16.2 的 `HttpConnection.Response.execute`
     * 真正用到的那几个成员(方法/头/输出流/响应码/头字段/输入流)。
     * 任何未实现的成员会抛而不是静默走网络 —— 宁可炸也不许出网。
     *
     * **请求是懒发的**:`connect()` 只置位,真正打出去要等到有人问响应
     * (`getResponseCode` / `getInputStream` / `getHeaderField*`)。
     * 这一条是踩出来的 —— jsoup 的 POST 是 `connect()` **之后**才往
     * `getOutputStream()` 写 body 的,急着在 connect 里发就会把 body 丢掉
     * (实测 `js-corpus-ea97f51e04` 报 `IllegalStateException: Already connected`)。
     */
    private class ReplayConnection(u: URL) : HttpURLConnection(u) {
        private var bodyOut: ByteArrayOutputStream? = null
        private var performed = false
        private var responseBytes: ByteArray = ByteArray(0)
        /** 展平成 (名, 值) 序对:同名多值(典型 Set-Cookie)各占一个下标 */
        private var responseHeaders: List<Pair<String, String>> = emptyList()

        /**
         * 请求头的快照。**必须在置 `connected` 之前取** ——
         * `URLConnection.getRequestProperties()` 在已连接后是抛
         * `IllegalStateException("Already connected")` 的。
         */
        private var sentHeaders: Map<String, List<String>> = emptyMap()

        override fun connect() {
            if (connected) return
            sentHeaders = requestProperties
            connected = true
        }

        private fun perform() {
            if (performed) return
            connect()
            performed = true
            val builder = Request.Builder().url(url.toString())
            sentHeaders.forEach { (name, values) ->
                values.forEach { builder.addHeader(name, it) }
            }
            val bytes = bodyOut?.toByteArray()
            val body = if (bytes != null || doOutput) {
                (bytes ?: ByteArray(0)).toRequestBody(null)
            } else {
                null
            }
            builder.method(method, body)
            val response = ReplayHttp.executeRaw(builder.build())
            responseCode = response.code
            responseMessage = response.message
            responseBytes = response.body.bytes()
            responseHeaders = buildList {
                response.headers.names().forEach { name ->
                    response.headers.values(name).forEach { add(name to it) }
                }
            }
        }

        override fun getOutputStream(): OutputStream =
            bodyOut ?: ByteArrayOutputStream().also { bodyOut = it }

        override fun getInputStream(): InputStream {
            perform()
            if (responseCode >= 400) throw IOException("HTTP $responseCode")
            return ByteArrayInputStream(responseBytes)
        }

        override fun getErrorStream(): InputStream? {
            if (!performed) return null
            return if (responseCode >= 400) ByteArrayInputStream(responseBytes) else null
        }

        override fun getResponseCode(): Int {
            perform()
            return responseCode
        }

        override fun getResponseMessage(): String? {
            perform()
            return responseMessage
        }

        override fun getHeaderFields(): Map<String, List<String>> {
            perform()
            return responseHeaders.groupBy({ it.first }, { it.second })
        }

        override fun getHeaderField(name: String): String? {
            perform()
            return responseHeaders.lastOrNull { it.first.equals(name, ignoreCase = true) }?.second
        }

        // jsoup 按下标遍历响应头(HttpConnection.Response.processResponseHeaders):
        // key 与 val 同时为 null 才停,故越界一律 null
        override fun getHeaderFieldKey(n: Int): String? {
            perform()
            return responseHeaders.getOrNull(n)?.first
        }

        override fun getHeaderField(n: Int): String? {
            perform()
            return responseHeaders.getOrNull(n)?.second
        }

        override fun disconnect() = Unit

        override fun usingProxy(): Boolean = false
    }
}
