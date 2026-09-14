// from: 文学吧 .ruleSearch.bookList
key = java.get("key");
url = "https://www.wenxue88.com/zz/index.html"
java.setContent(String(java.ajax(url)).replace(/href="/g,'href="/zz/'));
lis = java.getElements("@@class.zw_txt").toArray();
list = result.toArray().concat(lis);
re = new RegExp(key);
html = "";
for(i in list){
	re.test(list[i])?html+=list[i]:""
	}
result = String(html)
