// from: 一个阅读 .bookSourceComment
do{
t=Date();t1=Number(t.charAt(17));t2=Number(t.charAt(19));t3=Number(t.charAt(20));t4=Number(t.charAt(22));t5=Number(t.charAt(23));
n=(t2+t4)%6*1000+t1*100+t3*10+t5;
n=2302
n=2189
r=java.ajax("http://m.wufazhuce.com/article/"+n.toString());
java.setContent(r);
}
while(r.includes("404 Not Found"));
//获取4个随机数其中第一个数小于6，大于5918无内容，并访问网址
chapter.title=java.getString('class.text-title@text');
//传出标题
result=java.getString('class.text-content@html');
java.log("🍊🍊"+result);
//获取正文内容
if(String(result).includes("<img")){result=String(result).match(/src\=\"[^"]+\"/g).map(x=>'<img src="'+x.replace(/\?imageView[^"]+/g,"")+'">').join("\n");
java.log("🍊")}
//判断正文是否有图片并处理
book.variable+="\n﹉ID："+n+"﹉﹉\n"+result;
book.variable;
