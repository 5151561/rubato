// from: 话本小说[api] .ruleSearch.wordCount
//用于自动生成发现规则
b=false;
if(b){
let api="/app/lib/listv2?packageId=ihuaben&page=\{\{page\}\}&pageSize=20&sortType=clickcount_days_7&tagId=0&tokenId=NTAyNDUwNTA6MTYyNjcyMzYyMzpjMGU3ODlhNzQ3OTZmYmIy&keyword=";
let words=['都市言情','古代言情','玄幻言情','校园言情','穿越言情','灵异言情','短篇','二次元','灵异','都市','玄幻奇幻','历史军事','武侠仙侠','游戏竞技','科幻末世','明星同人','动漫同人','游戏同人','影视同人','小说同人'];
var s=[];
words.map(o=>{
var i={"title":o,"url":api+o}
s.push(i);
})
java.log(JSON.stringify(s))
}
java.getString("$.wordcount")
