// from: 💗 女读小说 .ruleToc.nextTocUrl
if(!baseUrl.match(/_/)){
num=result.match(/i\s*<=\s*(\d+)/)[1]
for(var i=2,txt=[];i<=num;i++){
	txt.push(baseUrl.replace(/\.html/,'_'+i+'\.html'))
	}
txt.join("\r\n")
}
