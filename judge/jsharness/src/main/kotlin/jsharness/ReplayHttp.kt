// HTTP 录放的裁判侧回放客户端(js-host 套)。**复制自 harness/ReplayHttp.kt**
// (两个模块的 classpath 互斥,复制不共享 —— 与其余 jsharness 垫片同一约定),
// 并追加 `executeRaw`:jsoup 那条路径(java.get/head/post)不走 okhttp 拦截器链。
// key 规范化与回放循环的契约是 docs/http-snapshot.md(§2-§7);
// 被测侧等价物 rust/crates/{difftest/src/replay.rs, net/src/client.rs},
// 合成侧 tools/http_snapshot.py——各处必须逐字一致。
// 回放循环本身是 okhttp 5.4.0 真实调用链(HttpHelper 拦截器 +
// RetryAndFollowUpInterceptor.followUpRequest + BridgeInterceptor)的摘录。
package jsharness

import com.google.gson.JsonObject
import com.google.gson.JsonParser
import io.legado.app.constant.AppConst
import io.legado.app.help.config.AppConfig
import io.legado.app.help.http.CookieManager
import okhttp3.Headers
import okhttp3.MediaType.Companion.toMediaTypeOrNull
import okhttp3.Protocol
import okhttp3.Request
import okhttp3.Response
import okhttp3.ResponseBody.Companion.toResponseBody
import java.io.File
import java.io.IOException
import java.security.MessageDigest

object ReplayHttp {
    var root: File? = null

    /** §6 回落页:未命中时读 `<root>/_fallback/<name>.json` 的 response 段 */
    var fallback: String? = null

    /**
     * 差分观察面:本 case 发出的 (method, 规范化 URL) 序列。
     *
     * **每个 case 一份新表,并绑在跑它的那条线程上**。理由(2026-08-31 踩到):
     * 被判 `timeout` 的 case,`future.cancel(true)` 杀不掉它那条线程 —— 中断只在
     * 阻塞点生效,而野生书源的死循环一次都不检查。实测的那一条是
     * `nextContentUrl` 从 `baseUrl` 自增页码,而**回落页每张都带「下一页」**,
     * 于是 `BookContent` 的 `while (nextUrl.isNotEmpty() && !nextUrlList.contains(nextUrl))`
     * 永不收敛。那条僵尸线程继续发请求,把 hop 记到**后面几个 case 的账上**
     * (实测污染 19 个 case,全是「裁判凭空多出几跳」的假 FAIL)。
     *
     * 僵尸线程手里攥的是它自己那份旧表,写进去没人读;工作线程(`mapAsync`
     * 的并发翻页)没绑,回落到当前这份 —— 那正是它该记的地方。
     */
    val hops: MutableList<Pair<String, String>> get() = current

    @Volatile
    private var current: MutableList<Pair<String, String>> = mutableListOf()
    private val owned = ThreadLocal<MutableList<Pair<String, String>>>()

    /** 新 case 开张:换一份新表并绑在当前线程上(见 [hops])。取代旧的 `hops.clear()` */
    fun newCase() {
        val fresh = mutableListOf<Pair<String, String>>()
        current = fresh
        owned.set(fresh)
    }

    private fun addHop(method: String, url: String) {
        (owned.get() ?: current).add(method to url)
    }

    fun reset(snapshotRoot: File?) {
        root = snapshotRoot
        newCase()
    }

    /**
     * jsoup 路径的回放(`JsExtensions.get/head/post` 走 `Jsoup.connect(...).execute()`)。
     *
     * 与 [executeCall] 的差别:**不走 okhttp 的拦截器链**(不补 UA / Keep-Alive /
     * cookieJar,不跟进重定向)—— 那条链是 okhttp 的,jsoup 没有。key 计算与快照
     * 查找仍是同一份(契约 docs/http-snapshot.md),故两条路径共用一个快照库。
     *
     * 由 [ReplayUrlStream] 在 `HttpURLConnection` 那一层调用:jsoup 1.16.2 的
     * `HttpConnection.Response.execute` 用的就是 `URL.openConnection()`。
     */
    fun executeRaw(request: Request): Response {
        val response = lookup(request)
        addHop(request.method, request.url.toString())
        return response
    }

    fun executeCall(request: Request, followRedirects: Boolean = true): Response {
        // 应用层拦截器(HttpHelper 真身,每 call 一次):UA 默认/删除 + Keep-Alive
        val appBuilder = request.newBuilder()
        val ua = request.header(AppConst.UA_NAME)
        if (ua == null) {
            appBuilder.addHeader(AppConst.UA_NAME, AppConfig.userAgent)
        } else if (ua == "null") {
            appBuilder.removeHeader(AppConst.UA_NAME)
        }
        appBuilder.addHeader("Keep-Alive", "300")
        appBuilder.addHeader("Connection", "Keep-Alive")
        var req = appBuilder.build()

        var followUps = 0
        var redirectedFrom: Int? = null
        while (true) {
            // 网络拦截器(HttpHelper 真身,逐跳):CookieJar 标记 → 注入/回存
            val cookieJar = req.header(CookieManager.cookieJarHeader) != null
            var sent = req.newBuilder().removeHeader(CookieManager.cookieJarHeader).build()
            if (cookieJar) {
                sent = CookieManager.loadRequest(sent)
            }
            // 记 okhttp 规范化后的 URL 本身(两侧 URL 规范化的差异要在这里暴露)
            val response = lookup(sent)
            addHop(sent.method, sent.url.toString())
            if (cookieJar) {
                CookieManager.saveResponse(response)
            }
            val followUp = if (followRedirects) followUpRequest(req, response) else null
            if (followUp == null) {
                // okhttp 会把前一跳挂在 priorResponse 上(WebBook 用它判定重定向)
                redirectedFrom?.let { code ->
                    return response.newBuilder()
                        .priorResponse(
                            Response.Builder()
                                .request(sent)
                                .protocol(Protocol.HTTP_1_1)
                                .code(code)
                                .message("")
                                .build()
                        )
                        .build()
                }
                return response
            }
            if (++followUps > 20) throw IOException("too_many_redirects")
            redirectedFrom = response.code
            req = followUp
        }
    }

    // okhttp RetryAndFollowUpInterceptor.followUpRequest / buildRedirectRequest 摘录
    private fun followUpRequest(userRequest: Request, userResponse: Response): Request? {
        when (userResponse.code) {
            307, 308 ->
                if (userRequest.method != "GET" && userRequest.method != "HEAD") return null
            300, 301, 302, 303 -> Unit
            else -> return null
        }
        val location = userResponse.header("Location") ?: return null
        val url = userResponse.request.url.resolve(location) ?: return null
        if (url.scheme != "http" && url.scheme != "https") return null

        val requestBuilder = userRequest.newBuilder()
        val method = userRequest.method
        if (method != "GET" && method != "HEAD") {
            val maintainBody = userResponse.code == 307 || userResponse.code == 308
            if (!maintainBody) {
                requestBuilder.method("GET", null)
                requestBuilder.removeHeader("Transfer-Encoding")
                requestBuilder.removeHeader("Content-Length")
                requestBuilder.removeHeader("Content-Type")
            }
        }
        val old = userRequest.url
        if (old.scheme != url.scheme || old.host != url.host || old.port != url.port) {
            requestBuilder.removeHeader("Authorization")
        }
        return requestBuilder.url(url).build()
    }

    private fun lookup(sent: Request): Response {
        val (key, hostDir, normalizedUrl) = snapshotKey(sent)
        val rootDir = root ?: error("ReplayHttp.root 未设置")
        var file = File(File(rootDir, hostDir), "$key.json")
        if (!file.isFile) {
            // §6 回落页:配了就用它顶上,没配才报 miss
            val fb = fallback?.let { File(File(rootDir, "_fallback"), "$it.json") }
            if (fb == null || !fb.isFile) throw IOException("snapshot_miss:$key:$normalizedUrl")
            file = fb
        }
        val snap = JsonParser.parseString(file.readText()).asJsonObject
        val resp = snap.getAsJsonObject("response")
        val headersBuilder = Headers.Builder()
        var contentType: String? = null
        resp.getAsJsonObject("headers")?.entrySet()?.forEach { (name, value) ->
            if (value.isJsonArray) {
                value.asJsonArray.forEach { headersBuilder.add(name, it.asString) }
            } else {
                headersBuilder.add(name, value.asString)
            }
            if (name.equals("content-type", ignoreCase = true) && !value.isJsonArray) {
                contentType = value.asString
            }
        }
        val bodyBytes = resp.get("bodyBase64")?.takeIf { !it.isJsonNull }
            ?.let { java.util.Base64.getDecoder().decode(it.asString) }
            ?: ByteArray(0)
        return Response.Builder()
            .request(sent)
            .protocol(Protocol.HTTP_1_1)
            .code(resp.get("status")?.asInt ?: 200)
            .message("")
            .headers(headersBuilder.build())
            .body(bodyBytes.toResponseBody(contentType?.toMediaTypeOrNull()))
            .build()
    }

    // ---- key 计算(docs/http-snapshot.md §2-§4)----

    private val whitelist = setOf("user-agent", "referer", "content-type", "x-requested-with")

    private fun snapshotKey(sent: Request): Triple<String, String, String> {
        val (normalizedUrl, hostDir) = normalizeUrlForKey(sent.url.toString())
        val lines = mutableListOf(sent.method.uppercase(), normalizedUrl)

        val hashed = mutableListOf<Pair<String, String>>()
        for (i in 0 until sent.headers.size) {
            val name = sent.headers.name(i).lowercase()
            if (name in whitelist) hashed.add(name to sent.headers.value(i).trim())
        }
        // BridgeInterceptor:头缺 Content-Type 时按 body 补(生效 Content-Type,§7.1)
        val bodyType = sent.body?.contentType()?.toString()
        if (bodyType != null && hashed.none { it.first == "content-type" } &&
            sent.header("Content-Type") == null
        ) {
            hashed.add("content-type" to bodyType.trim())
        }
        hashed.sortBy { it.first }
        hashed.forEach { (k, v) -> lines.add("$k: $v") }

        val body = sent.body
        if (body == null) {
            lines.add("")
        } else {
            val buf = okio.Buffer()
            body.writeTo(buf)
            lines.add(sha256Hex(buf.readByteArray()))
        }
        val digest = MessageDigest.getInstance("SHA-256")
            .digest(lines.joinToString("\n").toByteArray(Charsets.UTF_8))
        val key = digest.copyOfRange(0, 16).joinToString("") { "%02x".format(it) }
        return Triple(key, hostDir, normalizedUrl)
    }

    private fun sha256Hex(bytes: ByteArray): String =
        MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }

    /** §3:key 用的 URL 规范化(输入已是 okhttp 规范化串)。返回 (URL, host 目录名) */
    fun normalizeUrlForKey(url: String): Pair<String, String> {
        val schemeSplit = url.split("://", limit = 2)
        val scheme = if (schemeSplit.size == 2) schemeSplit[0].lowercase() else ""
        val rest = if (schemeSplit.size == 2) schemeSplit[1] else url
        val tailIdx = rest.indexOfFirst { it == '/' || it == '?' || it == '#' }
        val authority = if (tailIdx == -1) rest else rest.substring(0, tailIdx)
        val tail = if (tailIdx == -1) "" else rest.substring(tailIdx)
        val hostPort = authority.substringAfterLast('@')
        val colon = hostPort.lastIndexOf(':')
        var host: String
        var port: String?
        if (colon != -1 && colon + 1 < hostPort.length &&
            hostPort.substring(colon + 1).all { it.isDigit() }
        ) {
            host = hostPort.substring(0, colon)
            port = hostPort.substring(colon + 1)
        } else {
            host = hostPort
            port = null
        }
        host = host.lowercase().trimEnd('.')
        val defaultPort = when (scheme) {
            "http" -> "80"
            "https" -> "443"
            else -> null
        }
        if (port == defaultPort) port = null

        val pathQuery = tail.substringBefore('#')
        val qIdx = pathQuery.indexOf('?')
        var path = if (qIdx == -1) pathQuery else pathQuery.substring(0, qIdx)
        if (path.isEmpty()) path = "/"
        val query = if (qIdx == -1) null else pathQuery.substring(qIdx + 1).ifEmpty { null }
        val sortedQuery = query?.split('&')?.sortedWith(byteOrder)?.joinToString("&")

        val sb = StringBuilder("$scheme://$host")
        var hostDir = host
        if (port != null) {
            sb.append(':').append(port)
            hostDir = "${host}_$port"
        }
        sb.append(path)
        if (sortedQuery != null) sb.append('?').append(sortedQuery)
        return sb.toString() to hostDir
    }
}

// 字节序比较(String 默认 compareTo 是 UTF-16 码元序,与 UTF-8 字节序在
// 增补平面上不同;query 段实际多为 ASCII,此处按契约用字节序)
private val byteOrder = Comparator<String> { a, b ->
    val ab = a.toByteArray(Charsets.UTF_8)
    val bb = b.toByteArray(Charsets.UTF_8)
    val n = minOf(ab.size, bb.size)
    for (i in 0 until n) {
        val c = (ab[i].toInt() and 0xff).compareTo(bb[i].toInt() and 0xff)
        if (c != 0) return@Comparator c
    }
    ab.size.compareTo(bb.size)
}
