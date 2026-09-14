// from: 台湾小说网 .ruleToc.nextTocUrl
var page=src.match(/第\d+.(\d+)頁.當前\d+條.頁/)[1];
if(page){
	url=[];
	for(i=2;i<=page;i++){
		url.push(baseUrl.replace(/-1/,"-"+i))
		}
		url;
	}
