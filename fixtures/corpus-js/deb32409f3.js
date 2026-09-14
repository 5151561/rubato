// from: ♛ 连城读书 #渊呀1107 .ruleContent.content
var javaImport = new JavaImporter();
javaImport.importPackage(
    Packages.java.lang,
    Packages.javax.crypto.spec,
    Packages.javax.crypto,
    Packages.android.util
);

with(javaImport){
    function decrypt(str){
        var key=SecretKeySpec(String("1701019k").getBytes(),"DES");
        var iv=IvParameterSpec(java.base64DecodeToByteArray("AQIDBAUGBwg="));
        var bytes=Base64.decode(String(str).getBytes(),2);
        var chipher=Cipher.getInstance("DES/CBC/PKCS5Padding");
        chipher.init(2,key,iv);
        return String(chipher.doFinal(bytes));
    }
}
String(decrypt(JSON.parse(result).DATA.CTXT)).replace(/{{title}}/,'')
