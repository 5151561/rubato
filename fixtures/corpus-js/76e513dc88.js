// from: 📚 言情书库 .ruleToc.chapterList
var n=result.match(/<b>1<\/b>\/<b>(\d+)<\/b>/)[1];
var list=[{k:'第1页',v:baseUrl}];
for(var i=2;i<=n;i++){
list.push({k:'第'+i+'页',v:baseUrl.replace(/\.html/,'_'+i+'.html')});
}
list
