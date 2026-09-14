// from: ⭐新顶点网 .searchUrl
u='http://www.ttwx.net/js/common.js';
n=org.jsoup.Jsoup.parse(java.ajax(u)).html();
v=n.match(/<form action="\\.quot;(\/\w+.php)/)[1];
body='ie=gbk&q='+key;
'http://www.ttwx.net'+v+'?'+body
