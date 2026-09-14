// from: 新浪小说网 .ruleBookInfo.tocUrl
var bid = "";
var mat = result.match(/bid=([^&"]*)/i);
if(mat){
bid = mat[1];
}
java.put("bid",bid);
var body = "bid="+bid+"&page_size=50&page=1";
var option={"charset": "utf-8","method": "POST","body": String(body)};
"https://book.sina.cn/dpool/newbook/bookv1/ajax/get_catalog.php"+","+JSON.stringify(option);
