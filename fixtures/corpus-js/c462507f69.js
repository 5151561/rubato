// from: 粉丝漫画 .ruleContent.content
var options = {
"headers": {"Referer": baseUrl}
}
java.getStringList("$.data.imageArray").toArray().map
(id=>'<img src="'+id+'" >').join("\n")
