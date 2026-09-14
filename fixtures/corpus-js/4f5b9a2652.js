// from: 📚 夜寒书库 .searchUrl
var url=source.getKey();

var html = java.ajax(url);

so = org.jsoup.Jsoup.parse(html).select('form[name=search]').attr('action');

url+so+","+JSON.stringify({
  "body": "searchkey={{key}}",
  "method": "post"
})
