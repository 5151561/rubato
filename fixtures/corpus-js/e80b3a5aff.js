// from: 🔰笔趣阁.pysmei .ruleBookInfo.author
try{
var javaImport = new JavaImporter();
javaImport.importPackage(
    Packages.java.lang,
    Packages.javax.crypto,
    Packages.javax.crypto.spec,
    Packages.android.util
);
with(javaImport){
        var key=SecretKeySpec(String("OW84U8Eerdb99rtsTXWSILDO").getBytes(),"DESede");
        var iv=IvParameterSpec(String("SK8bncVu").getBytes());
        var bytes=Base64.decode(String(result).getBytes(),2);
        var chipher=Cipher.getInstance("DESede/CBC/PKCS5Padding");
        chipher.init(2,key,iv);
        result= String(chipher.doFinal(bytes));
}}catch(e){result}
