// from: 📚 熊猫看书 .ruleToc.nextTocUrl
var a = 'https://anduril.xmkanshu.com/v3/book/get_last_chapter_list?bookid=@get:{book}&page=1&pagesize=100000&lastchapterid=';
var r = [];
for(var i=1;i<50;i++){
    r.push(a + parseInt(i*300));
}
r
