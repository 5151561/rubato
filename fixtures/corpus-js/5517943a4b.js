// from: 木兰花漫 .ruleSearch.bookList
//java.log(result)
doc = org.jsoup.Jsoup.parse(result)
$ = (a) => doc.select(a)
if (/function S2H/.test(result)) {
	function S2H(s){
        var r = [];
        for(var i=0;i<s.length/2;i++){
            r[r.length]= parseInt(s.substr(i*2,2), 16);
        }
        return r;
    }
    var str = S2H(String($('str').html()).replace(/\s/ig,''));
    var key = S2H(String(cookie.getKey(source.getKey(),'key')));
    var x = 0, ret = "";
    for(var i=0;i<str.length;i++){
        var d = (str[i] + key[x]);
        if(d>255)d=d%255;
        ret+='%'+('0'+d.toString(16)).substr(-2,2);
        x++; if(x>=key.length) x=0;
    }
    var html = decodeURIComponent(decodeURI(ret));
    $('str').html(html);
}
//java.log(doc)
doc;
