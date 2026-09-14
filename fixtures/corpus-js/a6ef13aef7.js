// from: 🔥 腐小说 .ruleToc.chapterList
var n=result.match(/共(\d+)页/)[1];
var list=[{k:'1',v:baseUrl}];
for(var i=2;i<=n;i++){
list.push({k:''+i,v:baseUrl.replace(/\.html/,'_'+i+'.html')});
}
list
