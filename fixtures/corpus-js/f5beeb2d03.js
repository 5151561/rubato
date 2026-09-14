// from: 📚 玄幻文学 .ruleContent.nextContentUrl
if (result.indexOf("下一页") > -1) {
mat = baseUrl.match(/\/\d+(_(\d+))?\.html/);
page = Number(mat[2]||1)+1;
baseUrl.replace(/\/(\d+)(_(\d+))?.html/,'/$1_'+page+'.html');
}
