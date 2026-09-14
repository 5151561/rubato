// from: ⌨趣书网(qubook) .ruleToc.chapterList
var n=result.match(/<b>1<\/b>\/<b>(\d+)<\/b>/)[1];

var list=[{text:'第1页',href:baseUrl}];

for(var i=2;i<=n;i++){
list.push({text:'第'+i+'页',href:baseUrl.replace(/\.html/,'_'+i+'.html')});

}
list
