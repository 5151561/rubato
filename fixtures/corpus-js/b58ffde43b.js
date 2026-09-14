// from: 晋江文学① .ruleSearch.lastChapter
last=JSON.parse(java.ajax('http://android.jjwxc.net/androidapi/chapterList?novelId='+java.get('id'))).chapterlist;
if(last){
last=last[last.length-1];
vip=last.isvip!=0?"🔒":'';
date=last.chapterdate;
chapter=last.chaptername;
chapterid=last.chapterid;
result=vip+chapterid+'.'+chapter+' • '+date}
