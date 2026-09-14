// from: 酷安应用评论 .ruleExplore.wordCount
let num = Number("{{$.replynum}}");
result = `${num}回复${(num && '('+formatDate({{$.lastupdate}})+')') || ""}`
