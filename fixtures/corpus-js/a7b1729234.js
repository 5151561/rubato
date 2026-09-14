// from: 果冻小说 .ruleToc.nextTocUrl
if(/_1.shtml/.test(baseUrl)){
	var url=baseUrl;
	}else{
		var id=baseUrl.match(/\d+\/(\d+)/)[1]
		var go=baseUrl.replace(/html.+html/,"html/")
		var	url=go+id+"/"+id+"_1.shtml"
		var surl=String(url)
		}
page=result.match(/\(第\d+\/(\d+)页\)当前\d+条\/页/)[1];
list=[];for(i=2;i<=page;i++){list.push(surl.replace(/_1/,'_'+i))}
list
