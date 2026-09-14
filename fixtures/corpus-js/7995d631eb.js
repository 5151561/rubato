// from: 中文万维 .ruleToc.chapterList
let v = [],
		list = [];
JSON.parse(result).list.map($=>{

//分卷判定
		V = $.name;
		if(v[v.length-1]!=V&&!/^\s*$/.test(V)){
				v.push(V)
				list.push({
						name: '📖['+V+']📖',
						volume: true
					})
			}

$.bookChapters.map($=>{
		return list.push({
				name: $.name,
				url: `https://readbook-service-freebook.cread.com/cx/itf/chapterRead?bookId=${$.bookid}&chapterId=${$.id}`,
				info: `章节字数：${$.wordCount}　更新时间：${$.updateDate}`
			});
	});
});
v.length<2?list.filter($=>!$.volume):list
