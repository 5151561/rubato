// from: 🌐 就去看网 .ruleContent.nextContentUrl
if((r=java.getStringList((nx=java.get("next"))+"a:matches(第二[頁页]|下[一\\s]*[頁页]):not([href~=^javascript:|^#])@href||a:matches(下[一\\s]*[篇章回节節话話]):not([href~=^javascript:|^#])@href||a[href~=[_-]\\d+(/|\\.[a-z]+)?$]:has(i,img):not(:matches(\\S),[href~=^javascript:|^#])@href")).size()){
if(~String(book.tocUrl+(nextChapterUrl||'')).indexOf(r=String(r.get(r.size()-1)))||~r.indexOf(nextChapterUrl||String(book.tocUrl).replace(/.+(?=_\d+\/$)/,'')))r=null
}else r=null;
if(r){if(nx!='')chapter.putVariable("next",(n=r.replace(/\d+(?=[^\d]*$)/,it=>+it+1))==r?'':'a[href="'+n+'"]@href||');
r+java.get("动")}
