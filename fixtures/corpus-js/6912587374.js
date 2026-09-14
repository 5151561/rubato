// from: 📃UC小说 .exploreUrl
var cat1='都市,玄幻,仙侠,灵异,历史,游戏,科幻,武侠,奇幻,竞技';var list=[];
function getUrl(cats,url1){cats.split(',').forEach((i)=>{list.push(i+'::'+url1+i)})};
list.push('男→::');
getUrl(cat1,'http://read.xiaoshuo1-sm.com/novel/i.php?do=is_caterank&p=17&page={{page}}&onlyCpBooks=1&status=2&firstCate=');
list.join('\n')
