// from: 文学吧 .ruleContent.content
result = java.getString("@@class.hycolor.-1@html");
if(result == ""){
     html = java.ajax(baseUrl.replace(/index/,'js'));
     java.setContent(html);
     result = "❗️刷新正文跳转添加书籍页面❗️\n"+java.getString("@@tag.p@html"); java.startBrowser("https://www.coolapk.com/link?url="+java.encodeURI("legado://import/addToBookshelf?src="+baseUrl+",{origin:'https://www.wenxue88.com/'}"),"添加书籍《"+chapter.title+"》");
	}
result
