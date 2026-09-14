// from: 西瓜小说 .ruleToc.chapterList
url = `https://xgmf.zuanqianyi.com/glory/free/1152?a,`;

$ = JSON.parse(result).data;
$.chapterNameList.map((text,i)=>{

body = JSON.stringify({
	chapterId: $.chapterIdList[i],
	bookId: $.bookId
});

href = url+JSON.stringify({
  "body": body,
  "headers": {
    "signtype": "2"
  },
  "method": "POST"
});
		return {text:text,href:href}
	});
