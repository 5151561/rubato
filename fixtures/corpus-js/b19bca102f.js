// from: 晋江① .ruleToc.chapterUrl
$=result;
cookie=java.getCookie("http://m.jjwxc.net","sid");
chapterid=$.chapterid;
cookie=java.get('cookie');
vip=$.isvip!='0';
type=$.chaptertype=='1';
if(!vip){
if(!type){
result='https://app-cdn.jjwxc.net/androidapi/chapterContent?novelId='+baseUrl.match(/novelId=(\d+)/)[1]+'&chapterId='+chapterid
}else{
chaptername=$.chaptername;
result='http://www.baidu.com?wd='+result+book.name+'/'+chaptername}
}else{result="http://app.jjwxc.org/androidapi/chapterContent?novelId="+baseUrl.match(/novelId=(\d+)/)[1]+'&versionCode=191&token='+cookie+'&chapterId='+chapterid}
