// from: ㊣♛书耽▪︎API #渊呀 .searchUrl
/**
填写账号密码调试后获取 login_token 和 account，分别填入。
**/

login_name=""
passwd=""
login_token=""
account=""

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

if (login_name && passwd && !login_token && !account) {
    let jsonStr = {
        "login_name": login_name,
        "passwd": passwd,
        "app_signature_md5": "f73576612783f8ed8b68cdf73a56be94",
        "app_version": "2.1.6",
        "channel": "default"
    }
    let body = "secret_content=" + encode(JSON.stringify(jsonStr));
    let option = {
        "method": "POST",
        "body": String(body)
    };
    url = "https://app.shubl.com/signup/login," + JSON.stringify(option)
    resp = JSON.parse(decode(java.ajax(url)))
    if (resp.code == "100000") {
        java.log('❗️ 这是你的 login_token，请复制 ❗️：' + resp.data.login_token + '\n' + '❗️ 这是你的 account，请复制 ❗️：' + resp.data.reader_info.account)
    } else {
        java.log('❗️ ' + resp.tip + ' ❗️')
    }
} else {
    java.log('❗️ 如需看付费章节，请登录！ ❗️')
}

java.put('login_token',login_token!=""?login_token:"0f6bd1d063f202f71c3b84678027ce81")
java.put('account',account!=""?account:"萌友521068519938")
let jsonObj={"app_signature_md5":"f73576612783f8ed8b68cdf73a56be94","app_version":"2.1.6","channel":"default","order":"week_click","count":"15","category_type":"1","page":page-1,"key":key,"login_token":String(java.get('login_token')),"account":String(java.get('account'))}
let body = "secret_content="+encode(JSON.stringify(jsonObj));
let option = {"method": "POST","body": String(body)};
"https://app.shubl.com/bookcity/get_filter_search_book_list," + JSON.stringify(option)
