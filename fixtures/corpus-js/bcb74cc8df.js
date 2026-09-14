// from: 顶点小说[jsz] .ruleContent.content
str=java.get("java");
str=str.replace("/xiaoshuo/","articleid=");
str=str.replace("/","&chapterid=");
str=str.replace("_","&pid=");
str=str.replace(".html","");
var ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/77.0.3865.120 Safari/537.36";
    var headers = {"User-Agent": ua};
    var body = str;
    var option = {
        "charset": "utf-8",
        "method": "POST",
        "body": String(body),
        "headers": headers,
    };
str="https://www.jszxfx.com/api/reader_js.php," + JSON.stringify(option);
java.ajax(str);
