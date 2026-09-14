// from: 蛋文库 .ruleToc.chapterList
var url=baseUrl.replace(/.html/,"_1.html")
var path="@@.pagination@a.-1@href";
var data=java.getString(path);
var page=data.match(/_(\d+)/)[1];
var list=[{text:"第1页",href:baseUrl}];
if(data&&page){
		for(var i=2;i<=page;i++){
		list.push({text:`第${i}页`,href:url.replace(/_1/,"_"+i)})
		}
		list;
	}else{
		java.toast("目录获取失败，未获取到目录，可检查网络或稍后再试！")
		}
