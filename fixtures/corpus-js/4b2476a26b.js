// from: 一个阅读 .ruleContent.content
do{
t=Date();t1=Number(t.charAt(17));t2=Number(t.charAt(19));t3=Number(t.charAt(20));t4=Number(t.charAt(22));t5=Number(t.charAt(23));
//var t=new Date();
//year=t.getFullYear();
//java.log("🍊"+year);
n=(t2+t4)%6*1000+t1*100+t3*10+t5;
r=java.ajax("http://m.wufazhuce.com/article/"+n.toString());
java.setContent(r);
}
while(r.includes("404 Not Found"));
//获取4个随机数其中第一个数小于6，大概大于5919无内容，并访问网址
chapter.title=java.getString('class.text-title@text');
chapter.url="http://m.wufazhuce.com/article/"+n.toString();
//传出标题
s=String(java.getString('class.text-content@html'));
//获取正文内容
if(s.includes("<img")){s=s.replace(/<img [^>]*src\=\"([^"]+?)(\?imageView[^"]+)?\"[^>]*>/g,'<img src="$1">')}
//判断正文是否有图片并处理
else if(s.includes("<![endif]-->")){s=s.replace(/<!--\[if gte[\S\s]+<!\[endif\]-->/,"")}
if(source.getVariable()<=0||source.getVariable()=="NaN"||book.variable==null){
book.variable="";
source.setVariable(39);
}
else{
source.setVariable(source.getVariable()-1);
}
book.variable+="\n★"+chapter.title+"\n"+s;
book.variable;
