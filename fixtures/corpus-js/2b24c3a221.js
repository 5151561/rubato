// from: 晋江① .ruleBookInfo.lastChapter
try{last=JSON.parse(java.ajax('http://android.jjwxc.net/androidapi/chapterList?novelId='+baseUrl.match(/(\d+)/)[1])).chapterlist;
$=last[last.length-1];
vip=$.isvip?'💰':'';
chapterid=$.chapterid;
chaptername=$.chaptername;
date=$.chapterdate;
vip+chapterid+'、'+chaptername+' '+date}
catch(err){
java.log(err)
result="请刷新"
}
