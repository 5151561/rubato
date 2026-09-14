// from: 新浪小说网 .ruleToc.nextTocUrl
var pageNum=1;
var mat=src.match(/"total_page":(\d+)/);
if(mat){
pageNum=mat[1];
}
var list = [];
var bid = java.get("bid");
for(var i=2;i<=pageNum;i++){
var body = "bid="+bid+"&page_size=50&page="+i;
var option={"charset": "utf-8","method": "POST","body": String(body)};
var url = "https://book.sina.cn/dpool/newbook/bookv1/ajax/get_catalog.php"+","+JSON.stringify(option);
list.push(url);
}
list
