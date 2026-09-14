// from: 💡 起点总榜  .ruleBookInfo.tocUrl
var id = baseUrl.match(/book\/(\d+)/)[1];
java.put('id', id);
'https://druid.if.qidian.com/argus/api/v1/chapterlist/chapterlist?bookId='+id;
