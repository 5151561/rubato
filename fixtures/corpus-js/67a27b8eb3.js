// from: 💗 值得阅读 .ruleBookInfo.tocUrl
ua = "Mozilla/5.0 (iPhone CPU iPhone OS 7_1_2 like Mac OS X) AppleWebKit/537.51.2 (KHTML, like Gecko) Version/7.0 Mobile/11D167 Safari/9537.53"
     time=Math.round(new Date()/1000)
     sign=java.md5Encode("com.ruffianhankin.meritreader1"+time+"vhjJVz1St6tK7!8n#B0MqRIuE2Dh7!C#")
     pt = "1"
     Content="text/html charset=UTF-8"
     Connection="close"
     Accept="*/*"
     Origin="*"
     Headers="X-Requested-With"
     vary="Accept-Encoding"
    package="com.ruffianhankin.meritreader"
     version="3.8.2"
     channel="baidu_tg104"
     Cache="no-cache, no-store"
     headers = {"Content-Type":Content,"Connection":Connection,"Accept":Accept,"Access-Control-Allow-Origin":Origin,"Access-Control-Allow-Headers":Headers,"vary":vary,"User-Agent":ua,"package":package,"pt":pt,"version":version,"channel":channel,"time":String(time),"sign":String(sign),"Cache-Control":Cache}
option={"headers":headers}
pt=JSON.parse(java.ajax("https://book.pjxhmy.com/source/"+parseInt(java.getString('$.book_id')/1000)+"/"+java.getString('$.book_id')+".html,"+JSON.stringify(option))).data[0].site_path
"https://catalog.pjxhmy.com/"+pt
