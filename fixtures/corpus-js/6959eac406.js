// from: ▪︎✾柚免费耽美 .ruleSearch.bookList
var javaImport = new JavaImporter();
javaImport.importPackage(
    Packages.java.lang,
    Packages.javax.crypto.spec,
    Packages.javax.crypto,
    Packages.android.util
);

with(javaImport){
  function decode(word){
      let key=SecretKeySpec(java.base64DecodeToByteArray("UP8XDHB/5Z29QGFovGSxyPwn9egkxEazAPz6Uoo80Zc="),"AES");
      let iv=IvParameterSpec(java.base64DecodeToByteArray("AAAAAAAAAAAAAAAAAAAAAA=="));
      let chipher=Cipher.getInstance("AES/CBC/PKCS5Padding");
      let bytes=Base64.decode(String(word).getBytes(),2);
      chipher.init(2,key,iv);
      return String(chipher.doFinal(bytes));
    }
}

decode(result)
