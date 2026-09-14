// from: 起点中文网💰 .ruleBookInfo.intro
var J = org.jsoup.Jsoup.parse(result);
'<br>' + J.select('.detail .tags')
  .toArray()
  .map(el => '🏷️' + el.text()).join('  ') +
  '<br>' +
  J.select('.book-intro').html();
