// from: 博趣中文网 .ruleContent.content
if (/内容正在载入/.test(src)) {
    c2 = src.match(/c2="([^"]+)/)[1];
    url = src.match(/src="([^"]+)/)[1];
    j = java.ajax(source.getKey() + url);
    reg = /function ajax[\s\S]+?\}(?=Ajax)/;
    eval(String(j.match(reg)[0]))
    java.log(java.ajax(source.getKey() + ajax(c2)));
    h = java.ajax(baseUrl);
    java.setContent(h), h;
} else result;
