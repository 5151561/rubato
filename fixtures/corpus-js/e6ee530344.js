// from: 🔞 书耽 .ruleExplore.bookList
var javaImport = new JavaImporter();
javaImport.importPackage(
    Packages.java.lang,
    Packages.javax.crypto.spec,
    Packages.javax.crypto,
    Packages.android.util
);

with(javaImport){
  function encode(word){
      let key=SecretKeySpec(java.base64DecodeToByteArray("nvlrM3RT6n0iYj4I/zbGqisUGGMpy3UT84cNphYONC8="),"AES");
      let iv=IvParameterSpec(java.base64DecodeToByteArray("AAAAAAAAAAAAAAAAAAAAAA=="));
      let chipher=Cipher.getInstance("AES/CBC/PKCS5Padding");
      chipher.init(1,key,iv);
      return java.encodeURI(Base64.encodeToString(chipher.doFinal(String(word).getBytes()),Base64.NO_WRAP));
    }
  function decode(word){
      let key=SecretKeySpec(java.base64DecodeToByteArray("nvlrM3RT6n0iYj4I/zbGqisUGGMpy3UT84cNphYONC8="),"AES");
      let iv=IvParameterSpec(java.base64DecodeToByteArray("AAAAAAAAAAAAAAAAAAAAAA=="));
      let chipher=Cipher.getInstance("AES/CBC/PKCS5Padding");
      let bytes=Base64.decode(String(word).getBytes(),2);
      chipher.init(2,key,iv);
      return String(chipher.doFinal(bytes));
    }
}
response=null
if(!baseUrl.match(/get_daily_task_bonus/)){
category_type=baseUrl.match(/category_type=(\d+)/)?baseUrl.match(/category_type=(\d+)/)[1]:""
order=baseUrl.match(/order=(.+?)&/)?baseUrl.match(/order=(.+?)&/)[1]:""
is_paid=baseUrl.match(/is_paid=(\d)&/)?baseUrl.match(/is_paid=(\d)&/)[1]:""
up_status=baseUrl.match(/up_status=(\d)&/)?baseUrl.match(/up_status=(\d)&/)[1]:""
jsonObj={"category_type":category_type,"app_signature_md5":"f73576612783f8ed8b68cdf73a56be94","app_version":"2.1.6","channel":"default","order":order,"count":"15","is_paid":is_paid,"page":String(baseUrl.match(/page=(\d+)/)[1]-1),"up_status":up_status,"login_token":String(java.get('login_token')),"account":String(java.get('account'))}
java.log(JSON.stringify(jsonObj))
option={"method":"POST","body":"secret_content="+encodeURIComponent(encode(JSON.stringify(jsonObj)))}
url="https://app.shubl.com/bookcity/get_filter_search_book_list,"+JSON.stringify(option)
response=decode(java.ajax(url))
}else{
jsonObj={"app_signature_md5":"f73576612783f8ed8b68cdf73a56be94","app_version":"2.1.6","channel":"default","task_type":"1","login_token":String(java.get('login_token')),"account":String(java.get('account'))}
option={"method":"POST","body":"secret_content="+encodeURIComponent(encode(JSON.stringify(jsonObj)))}
url="https://app.shubl.com/reader/get_daily_task_bonus,"+JSON.stringify(option)
response=decode(java.ajax(url))
}

// 打印解密结果
//java.log(JSON.stringify(JSON.parse(response)))
response
