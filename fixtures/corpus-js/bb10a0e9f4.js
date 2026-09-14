// from: 🌐 就去看网 .ruleBookInfo.tocUrl
if(java.get("单")==''){
if(java.get("录")==java.get("目")){
r=org.jsoup.Jsoup.parse(result);

script=r.select(":matchesOwn(^$)>a[href^=javascript:]:matches(全文|章[節节]|目[錄录]):not(:matches(最新))");re=false;

if(!script.size()){
ba=(bas=baseUrl.replace(/\/$|\.[a-zA-Z]+$/,'')).match(/(http....[^/?]+)(?:([?/])(.*))?$/);b=ba[1];v=ba[3];

r.select("a[href~=[^a-z]page[^a-z]]:not(:matches([反正顺順逆倒]序|[全正]文|更多|全部|所有|章[節节]|作品|目[錄录]|列表)),a:not([href~=^(?!//)[^#:]+$|"+b.split(':')[1]+"]),:matchesOwn(\\S)>a,a[href~=javascript:|#],a:matches(^\\S{1,4}$):not(:matches([反正顺順逆倒]序|[阅閱][讀读]|查看|展[开開]|进入|[全正]文|更多|完整|全部|所有|目[錄录]|列表|章[節节]):not(:contains(分类)))").remove();

y=r.select("a[href~=(?i)catalog|contents|chapters|mulu|(^|[^a-z])ml|showchapter|(^|chapter[/_-]?)(more|list|all)|sort[/=_-]asc]");

if(!y.size()){b3='';

if(ba[2]=="?"){
b3='[href~=^[^?]+$],'
}else if(ba[3]){
b3=ba[3].replace(/[*$|?+\\\^\[\](){}]/g,'\\$0');

b3="[href~="+b3+"(\\.[^.]+|/\\d+(\\.[^.]+|/)?)?$],[href~=[/?]"+b3.replace(/[&/_-][^/_-]+$/,'')+".*$]:not([href~=[/?]"+b3.replace(/[/_-]/g,'[/_-]')+"]),"}

y=r.select("a:not("+b3+"[href~=(?i)(^|[^/])[?/].*((book|[^a-z])(info|case)[^a-z]|(cid|buy)[^a-z]|desc|coins|nutrition|review|ticket|update|(app|author|xiazai|down)(?!=))|/chapter|/index/|/d/],:matches((?i)^$|[0-9零〇一二三四五六七八九十百千万、，：；？！。…‘’“”（）()]|[票榜:：.]|推荐|排行|等级|说明|收藏|书评|简介|分[类卷]|简介|作者|手机|软件|应用|安装|客户|移动|pc|电脑|安卓|苹果|下载|最新|ap[kp]|ipa|plx|deb|exe|zip|rar|txt|epub))")}

if(y.size()){
ys=y.select("a[href~=(?i)catalog|contents|list|chapter|mulu|(^|[^a-z])ml|more|read|all]:matches([反正顺順逆倒]序|全文|章[節节]|目[錄录]):not(:matches(阅读)),a[href~=(?i)catalog|contents|chapter|(^|[^a-z])ml|mulu|read]:matches(更多|列表),a[href~=(?i)catalog|contents|list|chapter|mulu|(^|[^a-z])ml|more|all]:matches(^$),a:matches(^[^\\u4e00-\\u9fa50-9]*([反正顺順逆倒]序|全文(免[費费])?[阅閱][讀读]|(点击|查看|展[开開]|进入|返?回到?)*([全正]文|(更多|完整|全部|所有)?(章[節节]|(作品)?目[錄录])+)+(列表)?(\\s*(查看|展开)?更多)?)[^\\u4e00-\\u9fa50-9]*$)");zt=false;

if(!ys.size()){
ba=bas.match(/(\?(?:[^=]+=)+)(.+)$/)||bas.match(/(?:[^/_-][/_-]([^/._-]+))?[/_-]([^/._-]+)(?:\/index[^/]*)?$/);

if(xi=(id=ba[2]).match(/\?[^=]+=([^&]+)/)||id.match(/^[^\d%]*(\d{2,}|[1-9])$/))id=xi[1];
if(ba[1])if(xi=ba[1].match(/^[^\d%]*(\d{2,})$/))id=/^\d$/.test(id)||!xi[1].indexOf(id)?xi[1]:"("+id+"|"+xi[1]+")";

ys=y.select("[href~=(?i)^((.*//[^/]+/)?[^=.]*[^\\d=.])?"+id+"([?_-][a-z=_-]*0|\\.[^.]+|[/?]([^\\d]*|[^/\\d]*/?|(list|more|all)([=_-][a-z]*)?\\d+[^\\d]*)?)?(&.+)?$]:not(:contains("+(bs=String(book.name)[0])+"))");

if(!ys.size()){zt=true;
ys=y.select("[href~=(?i)^((.*//[^/]+/)?[^=.]*[^\\d=.])?(\\d+/"+id+"[_-]\\d+[^\\d]*|"+id+"[_-][a-z_-]*1[^\\d]*)$]:not(:matches(阅读|"+bs+"))")}}

if(ys.size()){
if((re=ys.select("[href~=(/|^)[^.]+$]")).size())ys=re;
re=String(ys.first().attr("href"));

for(x=1;x<ys.size();x++)if((xs=String(ys.get(x).attr("href"))).length>re.length)re=xs;

if(/(?:[2-9]\d*|1\d+)[^\d]*$/.test(re)){
if(!zt&&(q=re.match(/^(.*[^/])?[&?/].*(?:catalog(ue)?|contents|(?:show)?chapters?|mulu|ml|more|all|list|page)(?:[=_-][a-z]*)?\d+[^\d]*$/i))&&v==(q[1]||'').replace(/^http....[^/?]+/,''))zt=true;
if(zt)re=re.replace(/\d+(?=[^\d]*$)/,"☯1")}

}}}

re=String(!re?baseUrl:(java.put("基",baseUrl),/^\/[^/]/.test(re)?b+re:/^https?:/.test(re)?re:/^\/\//.test(re)?b.split('//')[0]+re:String(baseUrl).replace(/[^/]*$/,'')+re))}else re=baseUrl;

r=re.replace(/(?:[☯?&/_-][^\d?/&_-]*[01])+[^\d]*$/,"");
java.put("ba",r);
re=re.replace("☯","")}else re=baseUrl;

java.get("跳")==1||re==baseUrl&&/,/.test(book.bookUrl)?re+',{"webView":true}':re
