// from: 🎉 闪舞小说 .exploreUrl
sort=[];
push=(title,url,type)=>sort.push({
		title: title,
		url: url,
		style: {
				layout_flexGrow: 1,
				layout_flexBasisPercent: type
			}
	});
$$=(a,b,c)=>`https://m.35xss.com/shuku/${a}_${b}_${c}_{{page}\}.html`;
[["综合榜","0"],["总点击","allvisit"],["月点击","monthvisit"],["周点击","weekvisit"],["日点击","dayvisit"],["总推荐","allvote"],["月推荐","monthvote"],["周推荐","weekvote"],["日推荐","dayvote"],["总收藏","goodnum"],["字数榜","size"],["新入库","postdate"]].map([title,a]=>{
		push(title,$$(a,0,0),1);
		["全部分类","玄幻奇幻","武侠仙侠","都市生活","历史军事","游戏竞技","科幻未来","恐怖悬疑","其他类型","古代言情","现代言情","幻想奇缘","游戏情缘","浪漫青春","言情美文","科幻灵异","其他类型"].map((title,b)=>{
				["["+title+"]","连载","完本"].map((title,c)=>{
						push(title,$$(a,b,c),0.25);
					});
			});
	});
JSON.stringify(sort);
