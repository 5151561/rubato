// from: 🎉 西瓜小说 .ruleToc.chapterList
var obj = JSON.parse(result); 
var chapterNameList= obj.data.chapterNameList;
var chapterIdList= obj.data.chapterIdList;
var bookId = obj.data.bookId;
var length = parseInt(obj.data.chapterNameList.length);
var list = [];
var ret;
for(var i = 0; i < length; i++)
{ 	 list.push({"chapterName":String(chapterNameList[i]), "chapterId":String(chapterIdList[i])});
}
java.put("bookId",bookId);

list
