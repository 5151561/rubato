// from: 群小说网 .searchUrl
var html = java.ajax(source.getKey())

token = org.jsoup.Jsoup.parse(html).select('input[name=_token]').attr('value')

"/search,"+JSON.stringify({
  "body": `_token=${token}&keyword=${key}`,
  "method": "POST"
})
