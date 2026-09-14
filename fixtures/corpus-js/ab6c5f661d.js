// from: 来看文学 .ruleContent.content
if(result.match(/<title>\d+</)){
cookie.removeCookie(baseUrl)
result=java.ajax(baseUrl)
}
eval(String(source.bookSourceComment))
cc=String(cookie.getCookie(baseUrl)).replace(/\%2C[^;]+/g,"")
cookie.setCookie(baseUrl,cc)
java.setContent(result)
result
