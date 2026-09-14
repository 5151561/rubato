// from: 文趣阁 .ruleBookInfo.tocUrl
let bid=parseInt(java.getString('$.book_id'))
let subPath=parseInt(bid/1000)
"http://s.nshkedu.com/api/book/chapter/"+subPath+"/"+bid+"/list.json"
