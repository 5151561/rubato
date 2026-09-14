// from: 全本小说网 .searchUrl
var url=source.getKey();

var html = java.ajax(url);

so = org.jsoup.Jsoup.parse(html).select('form[name=t_frmsearch]').attr('action');

url+so+","+JSON.stringify({
  "body": "searchkey={{key}}",
  "method": "post"
})
