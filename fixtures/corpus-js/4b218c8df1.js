// from: 📚 期刊杂志 .ruleContent.content
//净化插入的随机字符串
var doc=org.jsoup.Jsoup.parse(result);
var text=doc.select("div.text>*").not("h3,p,figure").remove();
java.log('#净化1#:'+text);
text=doc.select("div.text>p>*").remove();
java.log('#净化2#:'+text);
text=doc.select("div.text>figure>*").not("img,figcaption").remove();
java.log('#净化3#:'+text);
//净化完毕

//标题和图片标注格式化
result=String(doc.html()).replace(/<h3>(.+)<\/h3>/g,'【$1】').replace(/<figcaption>\s*(.+)\s*<\/figcaption>/g,'〔$1〕').replace(/data-action="zoom"/g,'');
