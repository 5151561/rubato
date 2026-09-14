// from: 全本小说 .exploreUrl
sort=[];
push=(title,url,type)=>{
		return sort.push(JSON.stringify({
				title: title,
				url: url?url:"",
				style: {
						layout_flexGrow: 1,
						layout_flexBasisPercent: type
					}
			}))
	}
$$=(a,b,c)=>b?`http://119.45.176.116:5006/localBookListByCategory?ps=20&length=${a}&pn={{page-1}\}&cid=${b}&order=${c}&status=2`:`http://119.45.176.116:5006/recList?gender=${a}&pn={{page-1}\}`;
[
		["男",[["都市娱乐",1],["玄幻奇幻",5],["武侠仙侠",6],["历史军事",4],["悬疑推理",29],["科幻游戏",28]]],
		["女",[["现代言情",2],["古代情缘",9],["灵异爱情",34],["玄幻奇幻",38],["耽美同人",11],["短篇小说",33],["其他小说",31]]]
].map(([title,list],gender)=>{
		gender++
		push("༺ˇ»`ʚ "+title+"生频道 ɞ´«ˇ༻",$$(gender,null,null),1);
		list.map([title,b]=>{
				push("༺ "+title+" ༻",$$(0,b,1),1);
				["热门","评分","字数"].map((title,c)=>{
						c++;
						["["+title+"]","短篇","中篇","长篇"].map((title,a)=>{
								push(title,$$(a,b,c),a==0?0.25:0.15);
							});
					});
			});
	});
"["+sort.toString()+"]";
