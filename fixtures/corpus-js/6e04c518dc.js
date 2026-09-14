// from: ♛ 拂袖 #渊呀1107 .searchUrl
/*调试->右上角的小虫子，随便搜一个词*/
/*Token写了东西的话要删掉才能进行下面步骤：
①填上电话号码点调试会获取验证码；
②填上验证码点调试会获取token；
③把token填在Token【目录的章节url那也有个token记得也要填，不填的话从发现点的书是没有登录的】；
④把电话号码跟验证码的值删了【❗️填了电话号码会一搜索就发验证码】，保存书源*/
/*如登录过期需要重新登录，请删除以前填写的token再调试获取验证码，重复前面的步骤*/
function sign(list){
    var sb='2bb6ffe20b1eff00';
    list=list.sort();
    for(var i=0;i<list.length;i++){if(i==0){sb+=list[i];}else{sb+="&";sb+=list[i];}}
    sb+="cabf8a3adca20e831d7e96267f709032";
    return String(java.md5Encode(sb)).toUpperCase();
}

电话号码="";
验证码="";
Token="";


mobile=电话号码;
code=验证码;
time=Math.round(new Date()/1000);
if(mobile.match(/^\d{11}$/) && !code && !Token){
list=["mobile="+mobile,"packageName=com.fuxiuyuedu.fuzReader","time="+time,"token=","marketChannel=vivo","appId=","sysVer=7.1.2","osType=2","udid=1f705dac-2eec-3045-96be-713264df32c2","ver=1.0.0","product=1"];
body={"mobile":String(mobile),"sign":sign(list),"packageName":"com.fuxiuyuedu.fuzReader","time":time,"token":"","marketChannel":"vivo","appId":"","sysVer":"7.1.2","osType":"2","udid":"1f705dac-2eec-3045-96be-713264df32c2","ver":"1.0.0","product":"1"};
option={"method":"POST","body":JSON.stringify(body)};
url='http://open.zzzapp.cn/message/send,'+JSON.stringify(option);
java.log(JSON.parse(java.ajax(url)).msg=="success"?"验证码发送成功，请注意短信":"验证码获取失败，请稍候再试");
}else if(code && !Token){
list=["mobile="+mobile,"code="+code,"packageName=com.fuxiuyuedu.fuzReader","time="+time,"token=","marketChannel=vivo","appId=","sysVer=7.1.2","osType=2","udid=1f705dac-2eec-3045-96be-713264df32c2","ver=1.0.0","product=1"];
body={"mobile":mobile,"code":code,"sign":sign(list),"packageName":"com.fuxiuyuedu.fuzReader","time":time,"token":"","marketChannel":"vivo","appId":"","sysVer":"7.1.2","osType":"2","udid":"1f705dac-2eec-3045-96be-713264df32c2","ver":"1.0.0","product":"1"};
option={"method":"POST","body":JSON.stringify(body)};
url='http://open.zzzapp.cn/user/mobile-login,'+JSON.stringify(option);
json=JSON.parse(java.ajax(url));
java.log(json.msg=="success"?'❗️下面是你的Token，请复制❗️\n'+json.data.user_token:JSON.stringify(json));
}
java.put('token',Token);
list=["packageName=com.fuxiuyuedu.fuzReader","time="+time,"token=","marketChannel=vivo","appId=","page_num="+page,"sysVer=7.1.2","osType=2","keyword="+key,"udid=1f705dac-2eec-3045-96be-713264df32c2","ver=1.0.0","product=1"];
body={"sign":sign(list),"packageName":"com.fuxiuyuedu.fuzReader","time":time,"token":"","marketChannel":"vivo","appId":"","page_num":page,"sysVer":"7.1.2","osType":"2","keyword":key,"udid":"1f705dac-2eec-3045-96be-713264df32c2","ver":"1.0.0","product":"1"};
option={"method":"POST","body":JSON.stringify(body)};
"/book/search," + JSON.stringify(option);
