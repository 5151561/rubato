// from: ㊝豆瓣读书▪︎书评 #关耳 .ruleBookInfo.tocUrl
if(baseUrl.match(/author/)){result=baseUrl+'books?sortby=collect&format=pic'}else{if(baseUrl.match(/series|ebook/)){result=baseUrl}else{
b=baseUrl+'reviews';
a=java.ajax(b);
if(a.match(/书评\s*\(0\)/)){
result=baseUrl+"comments/"
}else{result=b}
}}
