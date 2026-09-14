// from: 🔖读书阁① .ruleContent.content
var javaImport = new JavaImporter();
javaImport.importPackage(
    Packages.java.lang,
    Packages.javax.crypto.spec,
    Packages.javax.crypto,
    Packages.java.util);
with (javaImport) {
    function decode(content) {
        var se = String("6CE93717FBEA3E4F").getBytes();
        var ivEncData = Base64.getDecoder().decode(String(content));
        var key = SecretKeySpec(se, "AES");
        var iv = IvParameterSpec(se);
        var chipher = Cipher.getInstance("AES/CBC/NoPadding");
        chipher.init(2, key, iv);
        return String(chipher.doFinal(ivEncData));
    }
}
enContent = JSON.parse(result).data.chapterInfo.content;
decode(enContent);
