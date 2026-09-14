// from: 漫客栈 .ruleContent.content
java.getStringList("$.data.page[*]image").toArray().map
(a=>'<img src="'+a+'">').join("\n")
