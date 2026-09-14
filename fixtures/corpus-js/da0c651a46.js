// from: 武林中文网 .ruleSearch.bookList
if(result.match(/Just a moment/)){
cookie.removeCookie(source.bookSourceUrl)
var x=java.get("url")
java.startBrowserAwait(x,"验证")
result=java.ajax(x)
	}else{
		result=result;
		}
	result;
