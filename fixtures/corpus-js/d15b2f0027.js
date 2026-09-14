// from: 笔书阁❶ .ruleExplore.bookUrl
let bid=parseInt(java.getString('$.book_id'))
let subPath=parseInt(bid/1000)
"http://s.damaoli.com/api/book/detail/"+subPath+"/"+bid+".json"
