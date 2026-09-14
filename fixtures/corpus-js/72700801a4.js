// from: 📃UC小说 .ruleBookInfo.init
var bookId=java.get('bid');
var encryptKey="37e81a9d8f02596e1b895d07c171d5c9",user_id="8000000",timestamp=parseInt((new Date).getTime()/1e3);
var o=bookId+timestamp+user_id+encryptKey;
var sign=java.md5Encode(o);
var list={'turl':'https://ocean.shuqireader.com/api/bcspub/qswebapi/book/chapterlist?_=&bookId='+bookId+'&user_id=8000000&sign='+sign+'&timestamp='+timestamp};list
