// from: 笔书阁❶ .ruleBookInfo.tocUrl
let bid=parseInt(java.getString('$.book_id'))
let subPath=parseInt(bid/1000)
"http://s.damaoli.com/api/book/chapter/"+subPath+"/"+bid+"/list.json"
