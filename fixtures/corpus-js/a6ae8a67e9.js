// from: 笔书阁❶ .ruleSearch.bookList
var javaImport = new JavaImporter();
javaImport.importPackage(
    Packages.java.lang,
    Packages.javax.crypto,
    Packages.javax.crypto.spec,
    Packages.android.util
);

with(javaImport){
    function decrypt(str){
        var key=SecretKeySpec(String("ZKYm5vSUhvcG9IbXNZTG1pb2").getBytes(),"DESede");
        var iv=IvParameterSpec(String("01234567").getBytes());
        var bytes=Base64.decode(String(str).getBytes(),2);
        var chipher=Cipher.getInstance("DESede/CBC/PKCS5Padding");
        chipher.init(2,key,iv);
        return String(chipher.doFinal(bytes));
    }
}
decrypt(JSON.parse(result).data.replace(/(\r\n)|(\n)|(\r)/g,''))
