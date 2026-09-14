// from: 趣书网/qubook .ruleToc.chapterList
page=java.getElement("@@class.pagination@a.0@b.1");
var ys = String(page).match(/\d+/);
var list = [{text:'第1页',href:baseUrl}];
if(ys>=2){
for(var i=2;i<=ys;i++){
var url = baseUrl.replace(/.html/,'')
var html = url+'_'+i+'.html';
list.push({text:'第'+i+'页',href:html});
}}
list;
