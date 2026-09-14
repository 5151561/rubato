// from: 📚 精品书城 .ruleToc.chapterList
eval(String(source.bookSourceComment));

//ajax
index_url = "https://jingpinshucheng.com/index.php?c=book&a=show.jsonp&callback=" + callback +"&book_id="+book_id+"&b="+base32(callback);
index_html = java.get(index_url,{Referer:baseUrl});

//列表拼接
list2_html = eval(String(index_html.body()));
java.getString('.list.1@html')+list2_html
