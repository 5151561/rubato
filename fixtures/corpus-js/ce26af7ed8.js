// from: 🌸大米小说 .ruleContent.content
let list = java.getElement("@@id.subFrom@input").toArray()
if(list.length){
  let body="";
  for(let i=0; i < list.length; i++){
	  body += list[i].attr("name")+"="+java.encodeURI(list[i].attr("value"))+"&"
	}
  let url = "https://www.damixs.co/get-page-data?"+Math.random();
  let option = {
	    "method":"POST",
	    "body":String(body).replace(/&$/,'')
	  }
  url += ","+JSON.stringify(option)
  let j = java.ajax(url);
  result += JSON.parse(j).data+"❎"
}
	result
