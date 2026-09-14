// from: 如文网 .exploreUrl
importClass(org.jsoup.Jsoup);
function get(www,css)
{
list=Jsoup.parse(java.ajax(www)).select(css).toArray().map(a=>a.text()+"::"+a.attr("href")).slice(1,25).join("&&").replace(/1\/&&/,"{{page}}/&&")+"&&全本::https://m.jybdsj.com/quanben/fenlei/{{page}}/";
return list;
}
result=get("https://m.jybdsj.com/fenlei/",".fixed ul li a")
