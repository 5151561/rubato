// from: ⚖️ 青羽阅读 .ruleToc.chapterUrl
bid='{{$.book_id}}';
cid='{{$.chapter_id}}';
time=Math.round(new Date()/1000);
t="i3aphc4zg3hmmcq0appId=&book_id="+bid+"&chapter_id="+cid+"&marketChannel=none&osType=2&packageName=com.qingyuleku.app&product=1&sysVer=7.1.2&time="+time+"&token=&udid=a71d7896-33c6-372b-8ac2-b66cce73b30e&ver=3.5.18je1sc1htum1hvutpso79oigrx8pv2gx";
sign=java.md5Encode(t).toUpperCase();

body={"sign":sign,"packageName":"com.qingyuleku.app","time":time,"token":"","marketChannel":"none","chapter_id":cid,"appId":"","sysVer":"7.1.2","book_id":bid,"osType":"2","udid":"a71d7896-33c6-372b-8ac2-b66cce73b30e","ver":"3.5.1","product":"1"}

option={"method":"POST","body":JSON
.stringify(body)};

"http://api.kuduwxw.com/chapter/text,"+JSON.stringify(option);
