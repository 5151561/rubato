// from: 🎉 酷我小说 .exploreUrl
sort = cache.getFile('KuwoNovels');
if(sort==null){
var fk = sk = 1,
sort1 = [],
sort2 = [];
push=(title,url,type1,type2)=>{
		json = JSON.stringify({
				title: title,
				url: url?url:"",
				style: {
						layout_flexGrow: 1,
						layout_flexBasisPercent: type1
					}
			});
		return  eval('sort'+type2+'.push(json)');
	}
$$=(freetype,category_id,fk,sk)=>`http://appi.kuwo.cn/novels/api/book/category/${freetype}?category_id=${category_id}&fk=${fk}&sk=${sk}&pi={{page}\}&ps=20`;

JSON.parse(java.ajax('http://appi.kuwo.cn/novels/api/book/categories')).data.map($=>{
		freetype = $.freetype
		if(freetype==3)return;

		push('༺ˇ»`ʚ'+$.freetype_name+'ɞ´«ˇ༻', null, 1, freetype);
		$.categories.map($=>{
				category_id = $.category_id
				push('༺ '+$.category_name+' ༻', $$(freetype,category_id,sk,fk), 1, freetype);
				["综合", "热门", "最新"].map((title,sk)=>{
						sk++
						['['+title+']', '完结', '连载'].map((title,fk)=>{
								fk++
								push(title, $$(freetype,category_id,sk,fk), 0.25, freetype);
							});
					});
			});
	});
sort = sort1.concat(sort2);
cache.putFile('KuwoNovels', sort, 6E6);
}
'['+sort.toString()+']'
