// from: 🗒 非常爱漫 .ruleContent.content
(function getImgList() {
    eval(result.match(/(var qTcms_Cur=[\S\s]+var qTcms_S_show_1=.*?;)/)[1]);
    var list = java.base64Decode(qTcms_S_m_murl_e).split("\\$qingtiandy\\$");
    var piclist = new Array();
    for (var i = 0; i < list.length; i++) {
        var s = list[i];
        piclist[i] =s;
    }
    return piclist;
}()).map(url=>
'<img src="'+url+'">').join("\n")
