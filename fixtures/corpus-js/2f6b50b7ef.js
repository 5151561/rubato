// from: 优书网 .ruleContent.content
function parseComments(n){
  for(var c=[],e=0;e<n.length;e++){
    var o=n[e];
    c.push("书友：".concat(o.createrId.userName,"\n时间：").concat(o.createdAt.slice(0,10),"\n评分：").concat("🌟".repeat(o.score),"\n评语：").concat(o.content.replace(/([。！？—…]”?)([\u4e00-\u9fa5][^。！？—…]{9})/g,"$1<br>$2")))
  }
  return c.join("\n————\n");
}
var data = JSON.parse(result);
data.data.comments.length === 0;
data.data.comments.length === 0 ? '还没有人对这本书发表评价哦！<br>': parseComments(data.data.comments);
