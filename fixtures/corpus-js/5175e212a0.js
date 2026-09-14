// from: 🌐 就去看网 .ruleToc.chapterList
if(java.get("单")==''){
src=org.jsoup.Jsoup.parse(src);

if((result=java.get("录"))==""){if(页=(result=src.select('a[href~=\\S]:matches(下[一\\s]*[页頁]|下[一二三四五六七八九十百千万〇零0-9]{2,}章):not([href~=^#|javascript:])')).size())result=result.first().attr('href')
}else{网=String(result).split("🌕");
for(i=3,页=+网[0],result=网[1]+2+网[3];i<=页;i++)result+='\n'+网[1]+i+网[3];
result=String(result)}

if(页)java.put("页",/,/.test(book.tocUrl)?result.split('\n').join(',{"webView":true}\n')+',{"webView":true}':result);

嗅=()=>String(src).match(/[\[(](["'])<[a-z]+[ >][^\[\]()]+<\/a>(?:[^\[\],()]*<\/[a-z]+>)?\1[\])]/);
转=it=>it.replace(/\\[Uu]([0-9a-zA-Z]{4})/g,(_,it)=>String.fromCharCode(parseInt('0x'+it)));
兜=()=>src.select(':matchesOwn(^$|[0-9〇一二三四五六七八九十])>a:matches(\\S):not(:has(*>*>:not(span)),[href~=(?i)passport|\\.aspx$|\\.php$|^https://[^/]+(/|index\\.[a-z]+)?$|(^|[^/])[?/].*((book|[^a-z])(info|case|page|reg|Game|Play)[^a-z]|buy[^a-z]|SystemInfo|hot|sort|desc|asc|top|coins|nutrition|review|ticket|update|(app|author|xiazai|down)(?!=))|/list\\d*([/_-][^/_-]+/?)?$|\\d+_\\d+_\\d+|target=iframe|https%],:matches((?i)^((点击|软件|应用|安装|客户|移动|手机|电脑|安卓|苹果|下载|阅读|pc|ap[kp]|ipa|plx|deb|exe|zip|rar|txt|epub)[\\s.端版]*(?=$|点击|软件|应用|安装|客户|移动|手机|电脑|安卓|苹果|下载|阅读|pc|ap[kp]|ipa|plx|deb|exe|zip|rar|txt|epub)|[A-Za-z0-9\\u4e00-\\u9fa5]?返回.*(简介|书页|目录)[A-Za-z0-9\\u4e00-\\u9fa5]?|[<>-]+|\\d+-\\d+章|书页|目录|简介|[上下首尾]([一\\s]*[页頁]|[\\s\\d零〇一二三四五六七八九十百]*章)|[↑\\[]?[倒正反逆顺順]序[↓\\]]?)$))');

if((zt=java.get("嗅"))!=''){
if(zt>0&&(嗅=嗅())){
src=嗅[0];
if(zt==2)src=转(src);
src=org.jsoup.Jsoup.parse(src)}
src.select(java.get("除")).remove();
if(java.get("兜")==1)src=兜();
src=src.select(java.get("查"))

if(!页){for(首=String(java.get("首")).split('\n'),ss=src.size(),i=0;i<ss;i++)if(首[i]!=src.get(i)){
if(i>1)src.subList(1,i).clear();break;}}

}else{
book.putVariable("除",除="meta,link,a:has(img),"+((zt=java.get("全")!=1)?"a[href$="+String(book.tocUrl).replace(/,\{"webView":true\}|^.{8}[^/]*/g,'')+"],":"")+(基=String(java.get("基")),基==''?'':"a[href$="+基.replace(/^.{8}[^/]*/,'')+"],")+"a[href~=javascript:|#|[a-z]+[A-Z][a-z]+Id[=_-]|[^/][/?&]sub[A-Z]],a:matches(^$|最新章节$|^[^\\u4e00-\\u9fa5A-Z0-9]*(正文|.{0,2}书架|(免费|在线|开始|立即|全文|从头|点击|正文)+[试阅]读|[^\\s\\d外内楔前后卷篇章]*(更新调整|[两一二三四五六七八九十]+连更|作者[:：给要有]|双倍月票|感言|推书|推[a-z0-9A-Z_\\u4e00-\\u9fa5-]+书|[求个请投点下张](月?票|收藏|订阅|推荐)|(感谢|作者)[^\\s]*(读者|书友|大家|各位)|[书点]评[^\\s]*活动|[没有空]更新|没时间更新|请个?假|关于本书|关于更新|打赏名单|起点活动)[^\\s]*)[^\\u4e00-\\u9fa5A-Z0-9]*$)");

book.putVariable("嗅",(基=嗅())?(src=org.jsoup.Jsoup.parse((zt=/\\[Uu]([0-9a-zA-Z]{4})/.test(基=基[0]))?转(基):基),zt)?2:1:0);
src.select(除).remove();

if(zt=java.get("全")!=1){
找=ll=null;
$=it=>(qc=ll,查=找,ll=src.select(找=it)).size()>14&&(查=it,src=re=ll);

if(!(((ck=java.get("ck"))!=""&&$("[href~="+ck+"(?!index(/|\.[a-z]+)?$)[^.?/_-][^&/_-]*/?$|/[vV][iI][pP][_-]?([Rr]ead|[Cc]hapter)|([A-Za-z]\\d+|\\d[A-Za-z]+|[A-Z][a-z]+|[a-z][A-Z]+){3,}[^/?&]*$]"))||$("[href~=^[a-z0-9]+(/|\\.[^./]+)?$]"))){
if(qc&&qc.size()>ll.size())找=查,ll=qc;

$=it=>(re=src.select(it)).size()&&(查=it,src=re);

if(!($("[data-cid]")||$("[href~=(?i)(^|[/_-])(chapter|read)+([_-]?id)?/[^/_-]+[/_-][^/_-]+]")||$("[href~=(?i)(^|[&?/_-](book|novel|comic|manhua|mh?)?)(chapter|read)+([_-]?id)?[?/=]]")||$("[href~=(?i)[&?/_-]cid[&?/_=-]]")||$("[data-href]"))){
src=兜();
book.putVariable("兜",1);

ba=(ba=String(java.get("ba"))).match(/(\?(?:[^=]+=)+)(.+)$/)||ba.match(/(?:[^/_-][/_-]([^/._-]+))?[/_-]([^/._-]+)(?:\/|\/index[^/]*|\.[^/.]+)?$/);

if(xi=(id=ba[2]).match(/\?[^=]+=([^&]+)/)||id.match(/^[^\d%]*(\d{2,}|[1-9])$/)||ba[1]&&ba[1].match(/^[^\d%]*(\d{2,})$/))id=xi[1];

$("[href~=([^\\d]|^)"+id+"[/_&-][^\\d]*\\d+]:not([href~="+id+"[^\\d]*$]),[href~=/[vV][iI][pP]|([A-Za-z]\\d+|\\d[a-zA-Z]+|[A-Z][a-z]+|[a-z][A-Z]+){3,}[^/?]*$|([^\\d]|^)"+id+"[/_&-][^\\d]*"+id+"(/|\\.[^.]+|&.+)?$]")||$("[title]")||$("[href~=/view/\\d+\\.[a-zA-Z]+$]")}
if(re.size()<ll.size())查=找,src=ll}}

if(!(zt&&re.size()))src=src.select('a'),查='a';
if(查)book.putVariable("查",查);
if(页)java.put("首",src)}

src}else if((list=java.get("单"))!=1){

if((网=String(list).split("🌕")).length>1){
for(i=+网[2],x=+网[0],j=2,list="<a href='"+baseUrl+"'>正文1</a>";i<=x;i++,j++)list+="<a href='"+网[1]+i+网[3]+"'>正文"+j+"</a>"}

org.jsoup.Jsoup.parse(list).select('a')

}else ["<a href='"+baseUrl+"'>正文</a>"]
