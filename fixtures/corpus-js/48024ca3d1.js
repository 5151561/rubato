// from: 🌟 优书书评 .ruleContent.content
function parseComments(n){
  for(var c=[],e=0;e<n.length;e++){
    var o=n[e];
    c.push("@".concat(o.createrId.userName,"   ").concat(o.createdAt.slice(0,10),"\n评分：").concat("🌟".repeat(o.score),"\n👍").concat(o.praiseCount,"    💬 ").concat(o.replyCount,"\n").concat(o.content))
  }
  return c.join("\n");
}
var data = JSON.parse(result);
data.data.comments.length === 0;
data.data.comments.length === 0 ? '还没有人对这本书发表评价哦！<br>': parseComments(data.data.comments);
