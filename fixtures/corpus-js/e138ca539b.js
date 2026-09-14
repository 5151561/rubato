// from: 源书库 .ruleContent.content
if(/_\d\.html/.test(baseUrl)){
//第二页利用javascript解密
try{java.base64Decode(src.match(/var mytxt.*?['"](.*?)['"];/)[1])}
catch(e){
//java.log(e);
//java.log(baseUrl);
"第二页解密失败！链接\n"+baseUrl
}
}
//第一页常规方式获取
else java.getString("id.booktxt@html")
