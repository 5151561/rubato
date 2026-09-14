// from: ♛ 连城读书 #渊呀1107 .ruleSearch.lastChapter
time=Math.round(new Date())
url="GEThttp://a.lc1001.com/app/info/bookcatabID={{$.KEYID}}consumerKey=LCREAD_ANDROIDisUpdate=0timestamp="+time+"uID=0XKrqBSeeEwgDy2pT"
body="consumerKey=LCREAD_ANDROID&timestamp="+time+"&sign="+java.md5Encode(encodeURIComponent(url))+"&bID={{$.KEYID}}&isUpdate=0&uID=0"
option={"method":"POST","body":String(body)}
java.ajax("http://a.lc1001.com/app/info/bookcata,"+JSON.stringify(option))
