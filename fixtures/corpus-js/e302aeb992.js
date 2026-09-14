// from: 网阅完本 .exploreUrl
sort=[];
push=(title,url,type)=>sort.push({
		title: title,
		url: url,
		style: {
				layout_flexGrow: 1,
				layout_flexBasisPercent: type
			}
	});
$$=(a,b)=>`/books/free-${a}-${b}.html<,?page={{page}\}>`;
[["最新入库","t"],["热度排行","h"],["章节数量","c"],["书籍字数","s"]].map([title,a]=>{
push("༺ˇ»`ʚ "+title+" ɞ´«ˇ༻",null,1);
["综合全部","玄幻奇幻","恐怖灵异","都市言情","古代言情","浪漫青春","武侠修真","乡村生活","穿越小说","历史军事","科幻末世",null,"现代文学","古典文学","游戏竞技","  二次元  ",null,null,null,"都市小说","外国文学","侦探推理"].map((title,b)=>{
if(title){
		if(b)b+=2;
		push(title,$$(a,b),0.25);
	}
});
});
JSON.stringify(sort);
