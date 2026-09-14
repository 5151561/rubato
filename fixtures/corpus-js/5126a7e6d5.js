// from: 【优品PPT】 .ruleBookInfo.downloadUrls
var url = java.getString("class.down-button@href")
var a = java.ajax("https://www.ypppt.com" + url)
url = String(a).match(/href\=\"(.*?)\"\>下载地址1/)
url[1]
