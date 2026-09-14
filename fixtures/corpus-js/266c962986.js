// from: 腐文小说 .ruleToc.nextTocUrl
var regex=/第\d+.(\d+)页.当前\d+条.页/;
var page=src.match(regex)[1];
var url=[];
for(var i=2;i<=page;i++){
	url.push(baseUrl.replace(/_1/,"_"+i))
	}
url;
