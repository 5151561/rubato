// from: 神仙书阁 .ruleContent.content
num=result.split('"_').length-1
for (var i=0;i<num;i++){
str=result.match(/(<span class="(_.*?)".*?span>)/)
str2=result.match(str[2]+':.*?"(.*?)"')[1].replace(/\\/,'%u')
result=result.replace(str[1],unescape(str2))
}
result.replace(/[\r\n]/g,'').replace(/.*最新章节！/,'').replace(/([^>]+)最快更新.*/,'')
