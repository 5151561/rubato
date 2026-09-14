// from: 🐑知乎文学 .ruleSearch.bookList
key=java.get("key")
cookie.removeCookie("https://zhihuwenxue.com")

a='https://zhihuwenxue.com/search/,{"body": "searchkey='+key+'&submit=","method": "POST"}'

b=java.ajax(a)
java.setContent(b)

c=java.getElements("class.bookbox@class.p10@class.bookinfo")
