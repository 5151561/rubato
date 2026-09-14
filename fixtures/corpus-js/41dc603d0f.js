// from: 笔书阁❷ .ruleBookInfo.tocUrl
let bid=parseInt(java.getString('$.book_id'))
let subPath=parseInt(bid/1000)
"http://s.dushuapp.net/api/book/chapter/"+subPath+"/"+bid+"/list.json"
