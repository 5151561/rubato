// from: 阅友小说 .exploreUrl
sort=[];
push=(title,url,type)=>sort.push({
		title: title,
		url: url,
		style: {
				layout_flexGrow: 1,
				layout_flexBasisPercent: type
			}
	});
push('༺ˇ»`ʚ排行榜单ɞ´«ˇ༻',null,1);
["综合榜","人气榜","完本榜","新书榜"]
.map((t2,a)=>{
		if(a==3)a=4;
		["男频","女频"].map((t1,b)=>{
				if(b){
						rankId = 291-a;
					}
				else{
						rankId = 280+a;
					}
				push(t1+t2, `@js:
u0 = "/goway/goread/app/rank/v4/searchBook"
u3 = "page="+page+"&psize=20&rankId=${rankId}"
eval(String(source.bookSourceComment))`, 0.4);
			});
	});
push("",null,1);
[["默认排序","readers"],["最新入库","createTime"]].map([title,orderBy]=>{
		push('༺ˇ»`ʚ'+title+'ɞ´«ˇ༻',null,1);
		[
		["男频",["都市人生","玄幻奇幻","武侠仙侠","军事历史","科幻末世","游戏体育",null,"悬疑灵异","脑洞大开"]],
		["女频",["现代言情","古代言情","幻想言情",null,"穿越时空","宫闱争斗","豪门总裁","婚恋爱情","经商种田"]],
		["图书",[null,"出版读物","文学小说","古代典籍","学习强国","名家名作",null,null,"史家专著"]]
		].map(([title,list],aid)=>{
				aid=Number((aid+1)+'100');
				push('༺ '+title+' ༻',null,1);
				list.map((title,bid)=>{
						if(title){
								push(title, `@js:
u0 = "/goway/goread/app/classify/v352/search"
u3 = "classifySecondList=${aid+bid}&orderBy=${orderBy}&page="+page+"&psize=20"
eval(String(source.bookSourceComment))`, 0.4);
							}
					});
			});
		push("",null,1);
	});
JSON.stringify(sort);
