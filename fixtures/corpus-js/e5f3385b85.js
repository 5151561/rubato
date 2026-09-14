// from: ㊣♛书耽▪︎API #渊呀 .ruleSearch.bookUrl
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
}

let jsonObj={"app_signature_md5":"f73576612783f8ed8b68cdf73a56be94","app_version":"2.1.6","channel":"default","book_id":{{$.book_id}},"login_token":String(java.get('login_token')),"account":String(java.get('account'))}
let body = "secret_content="+encode(JSON.stringify(jsonObj));
let option = {"method": "POST","body": String(body)};
"https://app.shubl.com/book/get_info_by_id," + JSON.stringify(option)
