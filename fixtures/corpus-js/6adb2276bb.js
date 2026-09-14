// from: 📚 有度中文 .ruleToc.chapterList
er=0;
//按卷分隔
sub=String(result).slice(1,-1).split(/, (?=<li class="volumes ell")/);
sm=java.get("sm");
count=0;
if(sub.length>=1)for(var i=0;i<sub.length;i++){
//捕获卷名
jm=sub[i].match(/\s*([^<>\n]+)\s*<\/li/);
if(jm&&!(jm[1].match(/正文/)&&sub.length==1)){
	
	
	
//补上卷名
//sub[i]=sub[i]
//.replace(/html([^>]*)><span([^>]*)>/g,'html$1><span$2>'+ ('').padStart(3, "")+jm[1]+' ').replace(/javascript\:([^>]*)><span([^>]*)>/g,'javascript:/'+jm[1]+'$1><span$2>'+('').padStart(3, "\u2000")+jm[1]+' ');



//按卷分块，插入卷名
sub[i]='<li><a href="https://translate.google.cn/#view=home&op=translate&sl=zh-CN&tl=en&text='+(++count)+'.'+sm+'/'+jm[1]+'"><span>★'+jm[1]+'★</span></a></li>'+sub[i];
}}
//合并结果
sub.join("")
