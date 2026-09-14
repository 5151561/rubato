// from: 中文看，看中文 .searchUrl
html = java.ajax(source.getKey())

token = org.jsoup.Jsoup.parse(html).select('input[name=_token]').attr('value')

"/search,"+JSON.stringify({
  "body": `_token=${token}&kw=${key}`,
  "method": "POST"
})
