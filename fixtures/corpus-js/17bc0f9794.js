// from: 当书网/89看书 .ruleToc.chapterList
var path="@@.last_page@href";
var data=java.getString(path);
var page=data.match(/page=(\d+)/)[1];
var url=baseUrl.replace(/$/,"&page=1");
var list=[{text:"第1页",href:baseUrl}];
if(data&&page){
	for(var i=2;i<=page;i++){
		var title=`第${i}页`;
		var link=url.replace(/\d+$/,i);
		list.push({text:title,href:link});
		}
		list;
	}
