// from: 晋江文学① .ruleBookInfo.lastChapter
last=JSON.parse(java.ajax('http://android.jjwxc.net/androidapi/chapterList?novelId='+baseUrl.match(/(\d+)/)[1])).chapterlist;
$=last[last.length-1];
vip=$.isvip?'🔒':'';
chapterid=$.chapterid;
chaptername=$.chaptername;
date=$.chapterdate;
vip+chapterid+'.'+chaptername+' • '+date
