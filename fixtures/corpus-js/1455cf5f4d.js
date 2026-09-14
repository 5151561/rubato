// from: 晋江① .ruleToc.chapterName
$=result;
title=$.chaptername;
title=String(title).replace(/^\s+/,'');
intro=$.chapterintro;
java.put("intro",intro);
vip=$.isvip!='0';
lock=$.islock!='0';
type=$.chaptertype=='1';
num=!type?'第{{$.chapterid}}章 ':'';
if(title.match(/[一二三四五六七八九十百千万\d]+\s*章|^\d+[、\.\s]|chapter\s*\d+/i)){
num=''
}else{num=num}
l=lock?'[锁]':'';
result=num+title+l;

//不显示卷名
result=!type?result:'';
