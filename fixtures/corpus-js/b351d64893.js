// from: 看书吧 .ruleContent.content
let id = result.match(/\{siteid:'(\d+)',bid:'(\d+)',cid:'(\d+)'\}/);
if (id) {
    let body = "siteid=" + id[1] + "&bid=" + id[2] + "&cid=" + id[3];
    let path = "novelsearch/chapter/transcode.html,";
    let option = {
        "method": "POST",
        "body": String(body)
    };
    java.ajax(String(source.bookSourceUrl + path + JSON.stringify(option)))
}
