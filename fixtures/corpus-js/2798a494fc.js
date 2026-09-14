// from: ♛ 连城读书 #渊呀1107 .ruleSearch.bookUrl
time=Math.round(new Date())
url="GEThttp://a.lc1001.com/app/info/bookindexbID={{$.KEYID}}consumerKey=LCREAD_ANDROIDlmID=1000timestamp="+time+"uID=0XKrqBSeeEwgDy2pT"
body="consumerKey=LCREAD_ANDROID&timestamp="+time+"&sign="+java.md5Encode(encodeURIComponent(url))+"&bID={{$.KEYID}}&lmID=1000&uID=0"
option={"method":"POST","body":String(body)}
"/app/info/bookindex," + JSON.stringify(option)
