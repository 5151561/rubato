// from: ⚖️ 青羽阅读 .ruleExplore.bookUrl
bid='{{$.book_id}}';
time=Math.round(new Date()/1000);
t="i3aphc4zg3hmmcq0appId=&book_id="+bid+"&marketChannel=none&osType=2&packageName=com.qingyuleku.app&product=1&sysVer=7.1.2&time="+time+"&token=&udid=a71d7896-33c6-372b-8ac2-b66cce73b30e&ver=3.5.18je1sc1htum1hvutpso79oigrx8pv2gx";
sign=java.md5Encode(t).toUpperCase();

body={"sysVer":"7.1.2","book_id":bid,"sign":sign,"packageName":"com.qingyuleku.app","osType":"2","time":time,"token":"","marketChannel":"none","udid":"a71d7896-33c6-372b-8ac2-b66cce73b30e","appId":"","ver":"3.5.1","product":"1"}

option={"method":"POST","body":JSON
.stringify(body)};

"http://api.kuduwxw.com/book/info,"+JSON.stringify(option);
