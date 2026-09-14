// from: 📚 有度中文 .ruleContent.content
let c=java.getString("id.TextContent@html");
if(baseUrl.match(/yodu.org/)){
result=c
}else{
result=baseUrl.match(/text=[^\.]*\.(.*)/)[1]
}
