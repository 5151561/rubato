// from: 📖💯读书阁🔖🅰 .searchUrl
option={"method":"POST","body":{"version":"2.0"}}
url="http://"+JSON.parse(java.ajax("http://www.zmtt.net/checkUpdate,"+JSON.stringify(option))).data.url
java.log(url)
//java.put("url",url)

opt={"method":"POST","body":{"keyword":"{{key}}"}}
url+"search,"+JSON.stringify(opt)
