// from: 🎉 爱看书八 .ruleToc.nextTocUrl
var page=result.match(/当前第\d+页，共(\d+)页，每页显示\d+章/)[1];
if(baseUrl.match(/1\/$/)){
	var url=baseUrl;
	}else{
		var go=baseUrl.replace(/.html/,"");
		var url=String(go+"/1/");
}
var list=[];
for(var i=2;i<=page;i++){
	list.push(url.replace(/1\/$/,i+"/"));
}
list;
