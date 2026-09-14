// from: 笔书阁❷ .ruleSearch.bookUrl
let bid=parseInt(java.getString('$.book_id'))
let subPath=parseInt(bid/1000)
"http://s.dushuapp.net/api/book/detail/"+subPath+"/"+bid+".json"
