// from: 霹雳书坊 .ruleToc.nextTocUrl
var page=src.match(/第\d+.(\d+)页.当前\d+条.页/)[1]
var url=[];
for(var i=2;i<=page;i++){
	url.push(baseUrl.replace(/_1/,"_"+i))
	}
url;
