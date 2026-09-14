// from: 晋江① .ruleContent.content
var javaImport = new JavaImporter();
javaImport.importPackage(
     Packages.java.lang,
    Packages.javax.crypto.spec,
    Packages.javax.crypto,
    Packages.java.util 
);
with(javaImport){
    let IV_PARAMETER = String("1ae2c94b");
    let ALGORITHM = "DES"; 
    let CIPHER_ALGORITHM = "DES/CBC/PKCS5Padding";
    let CHARSET = "utf-8";
    let password=String("KK!%G3JdCHJxpAF3%Vg9pN");


function decode(data){
let dks = new DESKeySpec(String(password).getBytes());
let keyFactory = SecretKeyFactory.getInstance("DES");
let secretKey = keyFactory.generateSecret(dks);
let cipher = Cipher.getInstance(CIPHER_ALGORITHM);
let iv = new IvParameterSpec(IV_PARAMETER.getBytes(CHARSET));
cipher.init(Cipher.DECRYPT_MODE, secretKey, iv);
return new String(cipher.doFinal(Base64.getDecoder().decode(String(data).getBytes(CHARSET))), CHARSET);

}
}
//DES解密结束

content=java.getElement("$.content");
saybody=java.getString("$.sayBody");
say=saybody!=""?"\n作者有话要说：\n"+saybody:"";
if(baseUrl.match(/token/)){
result=String(decode(content)+say);
}else{result=content+say}
