// from: 耽美文库 .ruleToc.chapterList
data=java.getString("@@a[title=Page]@text");
var page=data.match(/\/(\d+)/)[1];
var url=baseUrl.replace(/.html/,"_1.html")
var list=[{text:"第1页",href:baseUrl}];
if(data&&page){
	for(i=2;i<=page;i++){
		list.push({text:`第${i}页`,href:url.replace(/_1/,"_"+i)})
		}
		list;
	}
