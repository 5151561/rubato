// from: 大米小说 .ruleContent.nextContentUrl
if(/"\/get-page-next\?"/.test(String(result))){
url = "https://www.damixs.co/get-page-next?"+Math.random();

post = result.match(/data: {'v1':'([^']+)','v2':'([^']+)','v3':'([^']+)','v4':'([^']+)'},/);

body ="v1="+java.encodeURI(post[1])+"&v2="+java.encodeURI(post[2])+"&v3="+java.encodeURI(post[3])+"&v4="+java.encodeURI(post[4]);

option = {
	"method":"POST",
	"body":body
	}
	j = java.ajax(url+","+JSON.stringify(option))

JSON.parse(j).url.replace(/\s+/g,'+')
}
