// from: 爱去小说网 .ruleToc.chapterList
var page=src.match(/第\d+页.*共(\d+).+页/)[1];
var url=java.getString("@@text.首页.0@href")
var surl=String(url);
var list=[{text:"第1页",href:baseUrl}];
if(page){
	for(i=2;i<=page;i++){
		list.push({text:`第${i}页`,href:surl.replace(/page=1/,"page="+i)})
		}
		list;
	}
