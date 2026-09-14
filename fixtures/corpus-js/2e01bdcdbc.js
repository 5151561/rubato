// from: ♛ 读书阁▪︎API #渊呀 .searchUrl
option={"method":"POST","body":{"version":"2.0"}}
url="http://"+JSON.parse(java.ajax("http://www.zmtt.net/checkUpdate,"+JSON.stringify(option))).data.url
//java.put("url",url)
option={"method":"POST","body":{"keyword":key}}
url+"search,"+JSON.stringify(option)
