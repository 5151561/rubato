// from: 🌐 就去看网 .ruleBookInfo.name
j=String(book.customIntro).match(/^\{((?:[录原单动静直全跳逆字图]|\[[^\[\]]+\]|\d+#[^#]+#)+)\}/);

key=String((u=(baseUrl=String(baseUrl).replace(/,{[^{}]+}$/,'')).match(/^(.+)\?((?:[录原单动静直全跳逆字图]|\[[^\[\]]+\]|\d+#[^#]+#)+)$/))&&(baseUrl=u[1])&&j?j[1]+u[2]:u?u[2]:j?j[1]:'');

$=it=>(fn=r.select(it)).size()&&(fn=fn.first());

r=org.jsoup.Jsoup.parse(result);
r.select("script,noscript,style,head>:not(meta,title),footer,[class~=^foot],[id~=^foot],a:has(>:last-child:matchesOwn(^分类$))").remove();

m=String(r).replace(/(?:&nbsp;)+/g," ");

书=false;n=String(($('[property$=book_name]')&&(书=fn.attr('content')))||(function(){
if($('title')&&(书=String(fn.text()).replace(/^(简介页|详情页|目录页|正版|全本|免费阅读|[\s。.,_/|《「『【〖（(\[\])）〗】』」─—-]+)+/,""))){
for(x=0,c=r.select("h1,h2,h3,strong").eachText();x<c.size();x++)if(
(y=c.get(x))!=''&&(u=书.indexOf(y),~u&&u<4))return y}return 书}())||'请自行修改书名').replace(/(?!^)[^\u4e00-\u9fa5a-zA-Z0-9]*(?:笔趣阁|思路客|燃文|小说|漫画|手机)?(?:[.|,_/\s「『【〖（(\[\])）〗】』」。─—-]|(?:人工|机器|电脑)?校正|精校|完[整结]|加料|番外|未删节|简介|全[文本集]|下载|(?:小说|漫画|大全|正版(?:小说|漫画)?|免费|免费小说|免费漫画|免费全[文本]|在线|最[新快]|全部|手机|电脑)(?:全[文本集部]|大全|免费|在线|阅读|下载|章节|小说|更新|漫画|\.)|([^a-zA-Z0-9])(?:azw|mobi|epub|txt)(?![a-zA-Z0-9])|(?:最全)?(?:章节|目录|列表){2,}|更新章节最快|无广告|(?::顶点)?无弹窗|无防盗|小说网|手打全文|[纯全](?:手打|文字)|\s*by\s*(?=[\u4e00-\u9fa5]))[\S\s]*/i,"$1");

if($('[property$=author]')){
x=String(fn.attr("content")).replace(/^作\s*[者家][\s:：]*|(?!^)[/／｜|，,\s][^⚙️]*$/,"")
}else{
x=m.match(/>\s*([^>]+?)(?:\s*<\/[a-z]+>\s*|\s+)著\s*<|[\s\[\];?!,.()、，；？！。…─（）［］〖〗【】>《》](?:小说|漫画)?作\s*者(?![^>]+->)(?:[:：\s〖【（《［\[\(]|<[^it\/][^>]*>|<\/[^>]+>)+([^\s<">,，/／｜|\)\]］》）】〗]+)/);
x=x?x[1]?x[1]:x[2]:$('#author,.author')?String(fn.text()).replace(/(?!^)[/／｜|，,\s][^⚙️]*$/,""):""}
java.put("x",x);

c=(fn=r.select("meta[property~=category$]")).size()?String(fn.attr("content"))
.replace(/(?!^)\s*[，,./／｜|]\s*/,","):(fn=m.match(/(?:[\s\[\];?!,.()、，；？！。…─（）［］〖〗【】》]|<[^a/][^>]*>|<\/[^>]+>)(?:[分大]\s*类|类\s*[型别])(?:[:：\s]|<[^>]+>)+([^\s<."/／｜|>]+)/))&&fn[1];
if(c)java.put("v",c);

c=(fn=r.select("meta[property~=status$]")).size()?fn.attr("content"):(fn=m.match(/(?:[\s\[\];?!,.()、，；？！。…─（）［］〖〗【】》]|<[^a/][^>]*>|<\/[^>]+>)状\s*态(?:[:：\s]|<[^>]+>)+([^\s<."/／｜|>]+)/))&&fn[1];
if(c)java.put("s",c);

c=$('meta[property~=latest_chapter_name$]')?fn.attr("content"):(fn=m.match(/>(?:\s*[更最]\s*[新近])+(?:\s*章\s*节)?(?:[:：\s\[]|<[^>]+>)+(?!\s*(?:-|&gt;)\s*<|[:：\s0-9T年月日时分秒*-]{5,}<|[^:：]+[^章\s]\s*[:：]\s*<|更新(?:时间)?[:：])([^<"/／｜|\]>]+)/))&&fn[1];
if(c)java.put("z",c);

正=true;
if(key.length){
if(~key.indexOf("全"))java.put("全",1);
if((
u=key.match(/[^\[\]]+(?=\])/),
c=~key.indexOf("录"),
y=~key.indexOf("单"),
baseUrl=u?u[0]:baseUrl,
(c||u)&&(baseUrl=c||y?(baseUrl=String((c=baseUrl.match(/(.+[^\d])(\d+)([^\d]*)$/))[1]+1+c[3]),
c=c[2]+'🌕'+c[1]+'🌕'+2+'🌕'+c[3],
baseUrl):baseUrl)
)||~key.indexOf("直")
)java.put("目",1),正=1;
if(~key.indexOf("录"))java.put("录",c),正=1;
if(y||~key.indexOf("#")){
if(!u){
if((网=key.match(/(\d+)#([^#]+)/))
&&(尾=网[1],网=网[2].match(/^(.*[^\d])([12])([^/?\d]*)$/))
||(尾=r.select('a[href~=\\d[^/?\\d]*$]:matches(^(尾|末|最后一)[頁页篇章回节節话話]$)')).size()
&&(网=r.select('a:matches(^2$)')).size()
&&(网=String(网.first().attr('href')).match(/^(.*[^\d])([12])([^/?\d]*)$/))
&&(尾=String(尾.first().attr('href')).match(/\d+(?=[^/?\d]*$)/)[0])){
c=尾+'🌕'+网[1]+'🌕'+网[2]+'🌕'+网[3]
}else if((c=r.select('a:matches(^(\\d+|…+|\\.+)$)')).size()){
if((网=String(c).split(/<a[^>]+>[^\d<]+<\/a>/)).length==2){
for(c=网[0],尾=网[1],x=+网[1].match(/>([^<]+)/)[1],网=网[0].match(/href="([^"]*[^\d])(\d+)([^/?\d"]*)"[^>]*>([^<]+)<[^<]+$/),j=+网[4],i=+网[2]-j;j<x;j++)c+="<a href='"+网[1]+(j+i)+网[3]+"'>"+j+"</a>";
c+=尾}
}else c=1}
java.put("单",c),正=false}
if(~key.indexOf("跳"))java.put("跳",1);
if(~key.indexOf("逆"))book.setReverseToc(true);
else book.setReverseToc(false);
if((u=~key.indexOf("图"))||~key.indexOf("原"))java.put("原",1);
if(~key.indexOf("动"))java.put("动",',{"webView":true}');
else if(~key.indexOf("静"))java.put("静",1);
if(u||~key.indexOf("字"))java.put("文",u?1:2)
}else book.setReverseToc(false);

c=(fn=r.select("meta[property$=description][content~=\\S]")).size()?fn.get(fn.size()-1).attr("content"):(r.select(':matchesOwn([\\u4e00-\\u9fa5]{2,})>:not(br),:not(body,br,:matchesOwn([\\s\\S]{50}),:has(body,:matchesOwn([\\s\\S]{50})))').remove(),r.select(":matchesOwn(\\S)").text());

java.put("g",c=String(c).replace(/[\snbsp;]*(?:&nbsp;|\s){2,}|\s*([？！。]+[”」』\]\}\)）｝】〗〕〉]?)\s*/g,"$1　　").replace(/(?=　　)/g,"\n"));

if(c.length&&(c=c.match(/《([^《》]+)》(?!作品集)/))&&(c=c[1],书?~String(书).indexOf(c)&&~~c.indexOf(n):true))n=c;

if(正==1)m=java.ajax(baseUrl);
java.setContent(m,baseUrl);

if(正){
zl=java.getStringList("@css:[property$=latest_chapter_url]@content||:matches(^最新章节)>a:only-child:not([href~=^$|#|javascript:])@href||a:matches(^正文\\s*[\\d第一二三四五六七八九十〇零百千]|^[【《]?("+n+")?[\\s》】（\\u0028:：＿_－-]*(第[\\s0〇零]*[一1]\\s*[\\u4e00-\\u9fa5]|([\\u4e00-\\u9fa5]{2}阅读[（\\u0028:：＿_－-]?)?(0*1([）\\u0029.、:：_-]|$)|[〇零]*一([）\\u0029\\s.、:：_-]|$)))):not([href~=(^|[^/])/[vV][iI][pP]|([A-Za-z]\\d+|\\d[A-Za-z]+|[A-Z][a-z]+|[a-z][A-Z]+){3,}[^/?&_-]*$|^$|#|javascript:|"+(bas=baseUrl.replace(/\/$|\.[a-zA-Z]+$/,'')).match(/[^?/]+$/)[0].replace(/([*$|?+\\\^\[\](){}])/g,'\\$1')+"(?:[/_-]1)?(?:\\/|\\.[a-zA-Z]+)?$])@href||a:matches(^[^\\u4e00-\\u9fa5]*(免费|在线|开始|立即|全文|正文|从头)+[试阅]读[^\\u4e00-\\u9fa5]*$):not([href~=^$|#|javascript:])@href||a:matches(^0*1[^\\d]):not([href~=(^|[^/])/[vV][iI][pP]|([A-Za-z]\\d+|\\d[A-Za-z]+|[A-Z][a-z]+|[a-z][A-Z]+){3,}[^/?&_-]*$|^$|#|javascript:])@href");

if(zl.size()&&(ck=String(zl.get(0)).match(/^(.*\/\/[^/]+)?([/?]?[^/].+[?&/_-])[^&/_-]+\/?$/)))(jd=ck[1])&&(h=baseUrl.lastIndexOf('/',baseUrl.indexOf(jd.match(/(?:\.[^.]+){2,}$|[^./]+\.[^.]+$/)[0])))>8&&(
q=baseUrl.indexOf(':'),
(bas=java.get(baseUrl=baseUrl.slice(0,q+2)+baseUrl.slice(h),{})).statusCode()==200&&java.setContent(bas.body(),baseUrl)
),java.put("ck",ck[2])}
n
