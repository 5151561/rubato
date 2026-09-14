// from: ▪︎✾柚免费耽美 .ruleExplore.bookList
java.put('login_token','f37b91ca9d1348f6c307315433a71209')
java.put('account','西柚78264469064')
var javaImport = new JavaImporter();
javaImport.importPackage(
    Packages.java.lang,
    Packages.javax.crypto.spec,
    Packages.javax.crypto,
    Packages.android.util
);

with(javaImport){
  function encode(word){
      let key=SecretKeySpec(java.base64DecodeToByteArray("UP8XDHB/5Z29QGFovGSxyPwn9egkxEazAPz6Uoo80Zc="),"AES");
      let iv=IvParameterSpec(java.base64DecodeToByteArray("AAAAAAAAAAAAAAAAAAAAAA=="));
      let chipher=Cipher.getInstance("AES/CBC/PKCS5Padding");
      chipher.init(1,key,iv);
      return java.encodeURI(Base64.encodeToString(chipher.doFinal(String(word).getBytes()),Base64.NO_WRAP));
    }
  function decode(word){
      let key=SecretKeySpec(java.base64DecodeToByteArray("UP8XDHB/5Z29QGFovGSxyPwn9egkxEazAPz6Uoo80Zc="),"AES");
      let iv=IvParameterSpec(java.base64DecodeToByteArray("AAAAAAAAAAAAAAAAAAAAAA=="));
      let chipher=Cipher.getInstance("AES/CBC/PKCS5Padding");
      let bytes=Base64.decode(String(word).getBytes(),2);
      chipher.init(2,key,iv);
      return String(chipher.doFinal(bytes));
    }
}
category_type=baseUrl.match(/category_type=(\d+)/)?baseUrl.match(/category_type=(\d+)/)[1]:""
order=baseUrl.match(/order=(.+?)&/)?baseUrl.match(/order=(.+?)&/)[1]:""
is_paid=baseUrl.match(/is_paid=(\d)&/)?baseUrl.match(/is_paid=(\d)&/)[1]:""
up_status=baseUrl.match(/up_status=(\d)&/)?baseUrl.match(/up_status=(\d)&/)[1]:""

jsonObj={"app_signature_md5":"e7ad64b53de9d4d2dc02cc8d7e27bdb6","app_version":"1.1.6","channel":"6","count":"15","page":String(baseUrl.match(/page=(\d+)/)[1]-1),"order":order,"login_token":String(java.get('login_token')),"account":String(java.get('account'))}

java.log(JSON.stringify(jsonObj))
option={"method":"POST","body":"secret_content="+encodeURIComponent(encode(JSON.stringify(jsonObj)))}
url="https://xiyou.lfunv.com/bookcity/get_rank_book_list,"+JSON.stringify(option)
response=decode(java.ajax(url))

// 打印解密结果
java.log(JSON.stringify(JSON.parse(response)))
response
