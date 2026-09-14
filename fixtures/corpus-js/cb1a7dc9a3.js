// from: 梦幻小说 .ruleContent.content
list=java.getElements("@@id.txt@dd").toArray().sort((a,b)=>a.attr("data-id")-b.attr("data-id"));
list.join('\n')
