// from: 文趣阁 .ruleExplore.bookUrl
let bid=parseInt(java.getString('$.book_id'))
let subPath=parseInt(bid/1000)
"http://s.nshkedu.com/api/book/detail/"+subPath+"/"+bid+".json"
