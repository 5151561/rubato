// from: ▪︎✾柚免费耽美 .ruleExplore.bookUrl
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
}
let jsonObj={"app_signature_md5":"e7ad64b53de9d4d2dc02cc8d7e27bdb6","app_version":"1.1.6","channel":"6","from":"search","book_id":{{$.book_id}},"login_token":String(java.get('login_token')),"account":String(java.get('account'))}
let body = "secret_content="+encode(JSON.stringify(jsonObj));
let option = {"method": "POST","body": String(body)};
"https://xiyou.lfunv.com/book/get_info_by_id," + JSON.stringify(option)
