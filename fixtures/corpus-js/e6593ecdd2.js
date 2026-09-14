// from: 优书网 .ruleToc.chapterList
var data = JSON.parse(result).data;
var pageSize = data.comments.length;
var pageCount = Math.ceil(data.total/pageSize) || 1; // NaN
var list = Array.from(Array(pageCount).keys());
for(var i = 0; i <(pageCount>2 ? 2 : pageCount); i++){
  var index = list[i]+1;
  list[i] = {title:'第00' + index +'页', url: baseUrl.split('?')[0] + '?type=&page=' + index + '&t='+Date.now()};
}
list;
