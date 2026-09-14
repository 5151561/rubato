// from: 翠微居 .ruleContent.content
var id=baseUrl.match(/(\d+)\/(\d+)/);
var pid=baseUrl.includes("_")?baseUrl.match(/_(\d+)/)[1]:"1";
//java.log(pid)
var url=book.origin+"/api/reader_js.php,";
var data=`articleid=${id[1]}&chapterid=${id[2]}&pid=${pid}`;
var post=JSON.stringify({
  "body": String(data),
  "method": "POST"
});
var html=java.ajax(url+post);
java.getString("@@html",html);
