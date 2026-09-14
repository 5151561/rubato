// from: 趣又来 .searchUrl
var url = source.getKey();

var html = java.ajax(url);

token = org.jsoup.Jsoup.parse(html).select('input[name=_token]').attr('value');

url+"/search,"+JSON.stringify({
  "body": `_token=${token}&kw=${key}`,
  "method": "POST"
})
