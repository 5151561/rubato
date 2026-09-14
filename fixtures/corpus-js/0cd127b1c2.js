// from: ♛ 连城读书 #渊呀1107 .ruleBookInfo.lastChapter
time=Math.round(new Date())
url="GEThttp://a.lc1001.com/app/info/bookcatabID="+java.put('bid',java.getString('$.BID'))+"consumerKey=LCREAD_ANDROIDisUpdate=0timestamp="+time+"uID=0XKrqBSeeEwgDy2pT"
body="consumerKey=LCREAD_ANDROID&timestamp="+time+"&sign="+java.md5Encode(encodeURIComponent(url))+"&bID={{$.BID}}&isUpdate=0&uID=0"
option={"method":"POST","body":String(body)}
java.ajax("http://a.lc1001.com/app/info/bookcata,"+JSON.stringify(option))
