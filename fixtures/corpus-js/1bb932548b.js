// from: 🎨 漫客栈 .ruleContent.content
let list = JSON.parse(result).data.page.map((i)=>{
	return '<img src="'+i.image+'" />'
	})
String(list).replace(new RegExp(",","gi"),"")
