// from: 全本同人，标签 .searchUrl
var body="keyboard="+key+"&show="+"title"+"&classid="+"0";
var option={"method":"POST","charset": "gbk","body":String(body)};
a=java.ajax("http://qbtr.cc/e/search/index.php,"+JSON.stringify(option));
b=a.match(/searchid=(\d+)/);if(b==null){"http://qbtr.cc/e/search/index.php,"+JSON.stringify(option)}else{
"http://qbtr.cc/e/search/result/index.php?page={{page-1}}&searchid="+b[1]}
