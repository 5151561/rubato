// from: 🌐 就去看网 .ruleContent.content
dt=lr='';c=1;动=java.get("动");
if(动!=''&&!~baseUrl.indexOf(",")){
result=String(java.ajax(baseUrl+动))
}else if(java.get("静")==动)c=dt=2;

r=org.jsoup.Jsoup.parse(r1=result.replace(/&nbsp;/g,' '));
查=i=java.get("序");

if(java.get("文")==1){for(;c;c--){
d=["img[data-src],img[src~=[^a-z]cid[^a-z]]"
,"img:not([src~=(?i)^$|^javascript:|\\.gif|\\.png|[^a-z](cover|css|ic(on)?|load(ing|ed)?)[^a-z]])"
,"img[src~=(?i)\\.png]:not([src~=(?i)[^a-z](cover|css|ic(on)?|load(ing|ed)?)[^a-z]])"
,'img[src~=(?i)\\.gif]:not([src~=(?i)[^a-z](cover|css|ic(on)?|load(ing|ed)?)[^a-z]])'];

if(!(查!=""&&(查!=-1&&((lr=r.select(d[查])).size(),true)))){
$=it=>(lr=r.select(it)).size();
if($(d[i=0])||$(d[i=1])||$(d[i=2])||false)break;

if(c==2){dt=1;
r=org.jsoup.Jsoup.parse(java.ajax(baseUrl+',{"webView":true}'))
}else i=$(d[3])?3:-1}}
if(i==0)lr=String(lr).replace(/(?:src=['"][^'"]+['"] +)?data-/g,'');
if(查=="")book.putVariable("序",i);

}else{
sc=java.get("文")==2?
'[style~=(?i)text-align:center|(^| |;)color: *(rgb.(?!255[ ,]+255[ ,]+255)[\\d, ]*2\\d\\d|#(?=[a-f\\d]{3}([^a-f\\d]|$))(?!fff)[a-f\\d]*[d-f]|#(?=[a-f\\d]{4})(?!ffffff)([\\da-f]{2})*[d-f][\\da-f]|green|red|blue|yellow|purple|pink|brown)],script,noscript,style,header,footer,[class~=^foot|^head],[id~=^foot|^head],:has(>a):not(:has(p:matchesOwn(\\S),br)),a>*,:has(a):not(:matchesOwn([\\S\\s]{50,}),:has(:matchesOwn([\\S\\s]{50,}))),:matchesOwn([\\s\\S]{50})>:not(br,a,:matchesOwn([\\s\\S]{50})),:not(br,p,a,:matches([\\s\\S]{200}),:has(p,br,div:matchesOwn(，|。)+div:matchesOwn(，|。)),:has(p,br,div:matchesOwn(，|。)+div:matchesOwn(，|。)) :matchesOwn(\\S):not(:not(p,div,span:has(br))))'
:
'[style~=(?i)text-align:center|(^| |;)color: *(rgb.(?!255[ ,]+255[ ,]+255)[\\d, ]*2\\d\\d|#(?=[a-f\\d]{3}([^a-f\\d]|$))(?!fff)[a-f\\d]*[d-f]|#(?=[a-f\\d]{4})(?!ffffff)([\\da-f]{2})*[d-f][\\da-f]|green|red|blue|yellow|purple|pink|brown)],script,noscript,style,header,footer,[class~=^foot|^head],[id~=^foot|^head],:has(>a):not(:has(p:matchesOwn(\\S),br,img:not([src~=(?i)^$|^javascript:|[^a-z](css|ic(on)?|load(ing|ed)?)[^a-z]|/\\d+s\\.jpg]))),img[src~=(?i)^$|^javascript:|[^a-z](css|ic(on)?|load(ing|ed)?)[^a-z]|/\\d+s\\.jpg],a:not(:matches(^$)>img)>*,:has(a):not(img,:matchesOwn([\\S\\s]{50,}),:has(img,:matchesOwn([\\S\\s]{50,}))),:matchesOwn([\\s\\S]{50})>:not(img,br,a,:has(img),:matchesOwn([\\s\\S]{50})),:not(img,br,p,a,:matches([\\s\\S]{200}),:has(p,br,img,div:matchesOwn(，|。)+div:matchesOwn(，|。)),:has(img,p,br,div:matchesOwn(，|。)+div:matchesOwn(，|。)) :matchesOwn(\\S):not(:not(p,div,span:has(br))))';

d=[":matchesOwn(\\S):has(br):has(:matchesOwn(\\S):has(br))"
,":matchesOwn(\\S):has(br)"
,":has(>:matchesOwn(\\S):not(:has(*))+:matchesOwn(\\S):not(:has(*)))"
,":has(>:has(>p:only-child:matchesOwn(\\S):not(:has(*)))+:has(>p:only-child:matchesOwn(\\S):not(:has(*))))"
,"img"
,":matchesOwn(\\S)"];

try{for(查=i!=''?i:java.get("元");c;c--){

if(!(c>1&&(String(r.text()).length<400||r.select(':matchesOwn(内容未加载完成|关闭(阅读|小说)模式)').size()))){
r.select(sc).remove();

if(!(查!=""&&(lr=r.select(i?d[i==6?5:i]:查)).size()))for(i=0;i<6&&(lr=r.select(d[i]),i==4&&c==1?!lr.size():String(lr.text()).length<200);i++);
if(c<2||i<6)break;}

r=org.jsoup.Jsoup.parse(r2=String(java.ajax(baseUrl+',{"webView":true}')).replace(/(<[a-z]+)&nbsp;/g,'$1 '));
dt=r1.length==r2.length?2:1}

for(c=lr.first(),v=1;v<lr.size();v++)if(lr.get(v).parents().contains(c)){
lr.remove(v);
v--}else c=lr.get(v);

lr=String((c=lr.size()==2&&i<4)?String(lr.first().text()).length>String(lr.get(1).text()).length?lr.first():lr.get(1):(c=lr.size()==1)?lr.first():lr);

if(查==""){
if(c&&(查=lr.match(/<([a-z]+) ([^>]+)>/))&&(查[2]=查[2].match(/(?:id|class|style)=(?:"[^"]+"|'[^']+')|[^= ]+(?=="[^"]+"|'[^']+')/g))){
book.putVariable("元",查[1]+'['+查[2].join('][')+']')
}else book.putVariable("序",i)}

lr=lr.replace(/<([a-z]+)[^>]*"-\d+"[^>]*>[^<]+<\/\1>|[^<>]*<a[^<]+<\/a>[^<]*|&lt[; ]?\/?[a-z]+(?= |\/?&gt)(?:[ a-z=-]+|"[^"]+"|'[^']+')*\/?&gt[; ]?|[☯📑⚙️🌕︴]/g,"").replace(/\s+(?:\s|(?:(?:(?:n?b)?s)?p)?;)/g,"　　");

if(java.get("原")!=1)lr=(!lr.indexOf("　　")?lr.replace(/>(?!　　|\s*(?:(?:(?:n?b)?s)?p)?;)\s*(?=[^\s<>])/g,">︴"):lr).replace(/((?:[〖【『「（《〈〔［\[(][^〖【『「（《〈〔［\[()］〕〉》）」』】〗\]]*[)］〕〉》）」』】〗\]]\s*)*(?:第?\s*[一二三四五六七八九十百千万〇零0-9]+\s*[章节回話话：:.．,，、]*\s*)?{{n=(t=title.match(/\S+$/)[0].replace(/[*$|?+\\\^\[\](){}/]/g,".?")).replace(/^(正文[^\u4e00-\u9fa5A-Za-z]*|第?[一二三四五六七八九十百千万〇零0-9]+[章节回話话\s：:.．,，、]*)+/,""),n!=t&&/\S/.test(n)?"(?:第?\\s*[一二三四五六七八九十百千万〇零0-9]+\\s*[章节回話话：:.．,，、]*\\s*"+n+"|"+t+")":t}}(?:\s*[〖【『「（《〈〔［\[(][^〖【『「（《〈〔［\[()］〕〉》）」』】〗\]]*[)］〕〉》）」』】〗\]])*)/g,"⚙️$1⚙️")+"📑"
}catch(e){}}
if(dt)book.putVariable(dt==1?"动":"静",',{"webView":true}');lr
