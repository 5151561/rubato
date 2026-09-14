// from: 🌟 优书书评 .ruleSearch.lastChapter
var score = result =="0"? "【无人评价】": "好评率"+(result > 10 ? result:result*10)+"%";
"@get:{updateAt}".slice(0,10)+"▪" +parseInt({{$.countWord}}/10000) + '万字🐬' +"▪" +score;
