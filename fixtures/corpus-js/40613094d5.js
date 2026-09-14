// from: 大米小说 .ruleContent.content
list = java.getElement("@@id.subFrom@input").toArray()
if(list.length){
body="";
for(i=0;i<list.length;i++){
	body += list[i].attr("name")+"="+java.encodeURI(list[i].attr("value"))+"&"
	}
url = "https://www.damixs.co/get-page-data?"+Math.random();
option = {
	"method":"POST",
	"body":String(body).replace(/&$/,'')
	}
url = url+","+JSON.stringify(option)
j = java.ajax(url);
result = result + JSON.parse(j).data+"❎"
}else{
	result = result
	}
