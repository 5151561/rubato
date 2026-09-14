// from: ⚖️ 青羽阅读 .searchUrl
time=Math.round(new Date()/1000);
t="i3aphc4zg3hmmcq0appId=&keyword="+key+"&marketChannel=none&osType=2&packageName=com.qingyuleku.app&page_num="+page+"&product=1&sysVer=7.1.2&time="+time+"&token=&udid=a71d7896-33c6-372b-8ac2-b66cce73b30e&ver=3.5.18je1sc1htum1hvutpso79oigrx8pv2gx";
sign=java.md5Encode(t).toUpperCase();

body={"sign":sign,"packageName":"com.qingyuleku.app","time":time,"token":"","marketChannel":"none","appId":"","page_num":page,"sysVer":"7.1.2","osType":"2","keyword":key,"udid":"a71d7896-33c6-372b-8ac2-b66cce73b30e","ver":"3.5.1","product":"1"};
option={"method":"POST","body":JSON.stringify(body)};
"http://api.kuduwxw.com/book/search,"+JSON.stringify(option);
