// from: 新浪小说网 .ruleContent.content
var text = "";
var mat = result.match(/var chapterContent = "([^"]*?)";/);
if(mat){
text = mat[1];
}
unescape(text.replace(/\\/g,'%'))
