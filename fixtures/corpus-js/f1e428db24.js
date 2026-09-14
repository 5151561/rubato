// from: 好看漫画 .ruleContent.content
var options = {
"headers": {"Referer": baseUrl}
}
java.getStringList("$.data..url").toArray().map
(id=>'<img src="'+id+'" >').join("\n")
