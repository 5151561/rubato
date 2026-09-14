// from: 🗒 仙漫网站 .ruleContent.content
try{
eval(result.match(/(eval\([\s\S]+?)<\/script/)[1]);
host=result.match(/imgDomain = '(.*?)';/)[1];
picdata.map(a=>
'<img src="'+host+a+'">').join("\n")
}catch(e){}
