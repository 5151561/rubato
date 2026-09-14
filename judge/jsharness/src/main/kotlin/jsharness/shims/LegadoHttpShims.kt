// 差分垫片:help/http 里**挂不进真身**的那几处。
//
// M2c 起网络面走 HTTP 录放(jsharness/ReplayHttp,契约 docs/http-snapshot.md):
// StrResponse / CookieStore / CookieManager / RequestMethod 都已按原路径挂真身,
// 这里只剩三类 —— 与 :harness 的 HttpShims.kt 同形(**复制**,不共享):
// 1. OkHttpUtils.kt 的纯扩展(text/addHeaders/get/postForm/postJson)—— 逐字复刻
//    (真身文件里的 newCall* 走真网络,整文件挂不进来);
// 2. newCallResponse / newCallStrResponse —— 转发 jsharness/ReplayHttp;
// 3. SSLHelper —— jsoup 的 sslSocketFactory 要真类型。
//    (BackstageWebView 从前也在这里 —— 那个抛「未接」标记的桩已经拆掉:
//     真身按原路径挂进来了,平台那一半换成剧本,见 jsharness/WebViewStage.kt。)
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.http

import io.legado.app.utils.EncodingDetect
import io.legado.app.utils.Utf8BomUtils
import jsharness.ReplayHttp
import okhttp3.FormBody
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import okhttp3.ResponseBody
import java.nio.charset.Charset

object SSLHelper {
    // Jsoup.connect().sslSocketFactory(...) 要真类型;回放永不真正发请求
    val unsafeSSLSocketFactory: javax.net.ssl.SSLSocketFactory
        get() = javax.net.ssl.SSLSocketFactory.getDefault() as javax.net.ssl.SSLSocketFactory
}

// HttpHelper.getProxyClient:回放不区分代理,返回占位 client(从不真正发请求)
private val markerClient by lazy { OkHttpClient() }

fun getProxyClient(proxy: String? = null): OkHttpClient = markerClient

// help/http/Cronet.kt:真身走 Cronet 引擎(Android)。差分 AppConfig.isCronet=false,
// 该分支永不进入,拦截器恒 null。
object Cronet {
    val interceptor: okhttp3.Interceptor? = null
}

// ---- OkHttpUtils.kt 逐字复刻(纯 JVM 部分)----

suspend fun OkHttpClient.newCallResponse(
    retry: Int = 0,
    builder: Request.Builder.() -> Unit,
): Response {
    val requestBuilder = Request.Builder()
    requestBuilder.apply(builder)
    var response: Response? = null
    for (i in 0..retry) {
        // AnalyzeUrl 的 buildRequestClient 把 urlOption 的 followRedirects
        // 落在 client 配置上(readTimeout/callTimeout 对回放无影响),
        // 回放循环从这里取,复刻 okhttp 的「不跟进重定向」行为
        response = ReplayHttp.executeCall(requestBuilder.build(), followRedirects)
        if (response.isSuccessful) {
            return response
        }
    }
    return response!!
}

suspend fun OkHttpClient.newCallStrResponse(
    retry: Int = 0,
    builder: Request.Builder.() -> Unit,
): StrResponse {
    return newCallResponse(retry, builder).let {
        StrResponse(it, it.body.text())
    }
}

fun ResponseBody.text(encode: String? = null): String {
    val responseBytes = Utf8BomUtils.removeUTF8BOM(bytes())
    var charsetName: String? = encode

    charsetName?.let {
        return String(responseBytes, Charset.forName(charsetName))
    }

    //根据http头判断
    contentType()?.charset()?.let { charset ->
        return String(responseBytes, charset)
    }

    //根据内容判断
    charsetName = EncodingDetect.getHtmlEncode(responseBytes)
    return String(responseBytes, Charset.forName(charsetName))
}

fun Request.Builder.addHeaders(headers: Map<String, String>) {
    headers.forEach {
        addHeader(it.key, it.value)
    }
}

fun Request.Builder.get(url: String, encodedQuery: String?) {
    val httpBuilder = url.toHttpUrl().newBuilder()
    httpBuilder.encodedQuery(encodedQuery)
    url(httpBuilder.build())
}

fun Request.Builder.get(url: String, queryMap: Map<String, String>, encoded: Boolean = false) {
    val httpBuilder = url.toHttpUrl().newBuilder()
    queryMap.forEach {
        if (encoded) {
            httpBuilder.addEncodedQueryParameter(it.key, it.value)
        } else {
            httpBuilder.addQueryParameter(it.key, it.value)
        }
    }
    url(httpBuilder.build())
}

private val formContentType = "application/x-www-form-urlencoded".toMediaType()

fun Request.Builder.postForm(encodedForm: String) {
    post(encodedForm.toRequestBody(formContentType))
}

fun Request.Builder.postForm(form: Map<String, String>, encoded: Boolean = false) {
    val formBody = FormBody.Builder()
    form.forEach {
        if (encoded) {
            formBody.addEncoded(it.key, it.value)
        } else {
            formBody.add(it.key, it.value)
        }
    }
    post(formBody.build())
}

fun Request.Builder.postMultipart(type: String?, form: Map<String, Any>) {
    // multipart 上传不在本套被测面(upload 走 Phase 3)
    throw UnsupportedOperationException("差分桩:multipart 未实现")
}

fun Request.Builder.postJson(json: String?) {
    json?.let {
        val requestBody = json.toRequestBody("application/json; charset=UTF-8".toMediaType())
        post(requestBody)
    }
}
