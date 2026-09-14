// from: 全本小说 .ruleToc.chapterList
$ = JSON.parse(result).data;
		bid = $.book_id;
$.chapters.map($=>{
		$.url = `http://119.45.176.116:5006/chapterContent,{"body":{"book_id":${bid},"chapterIdList":"${$.id},"},"method":"POST"}`
		return $;
	});
