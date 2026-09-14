// from: 福书网 .ruleToc.chapterList
var page=src.match(/<b>(\d+)<.b>/)[1];
var url=baseUrl.replace(/.html/,"_0.html")
var list=[{text:"第1页",href:baseUrl}];
if(page&&list){
	for(i=1;i<page;i++){
		list.push({text:`第${i+1}页`,href:url.replace(/_0/,"_"+i)})
		}
		list;
	}else{
		java.longToast("目录获取失败，检查网络或稍后再试！")
		}
