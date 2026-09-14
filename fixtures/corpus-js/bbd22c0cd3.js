// from: 书香之家 .ruleBookInfo.tocUrl
ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 6_1_3 like Mac OS X) AppleWebKit/536.26 (KHTML, like Gecko) Version/6.0 Mobile/10B329 Safari/8536.25_com.shuxiangzhijia.xyz"
     time=Math.round(new Date()/1000)
     sign=java.md5Encode("com.shuxiangzhijia.xyz1"+time+"vhjJVz1St6tK7!8n#B0MqRIuE2Dh7!C#")
     pt = "1"
     Content="text/html charset=UTF-8"
     Connection="close"
     Accept="*/*"
     Origin="*"
     Headers="X-Requested-With"
     vary="Accept-Encoding"
    package="com.shuxiangzhijia.xyz"
     version="1.0.3"
     channel="kuaishou_tg101"
     Cache="no-cache, no-store"
     headers = {"Content-Type":Content,"Connection":Connection,"Accept":Accept,"Access-Control-Allow-Origin":Origin,"Access-Control-Allow-Headers":Headers,"vary":vary,"User-Agent":ua,"package":package,"pt":pt,"version":version,"channel":channel,"time":String(time),"sign":String(sign),"Cache-Control":Cache}
option={"headers":headers}
pt=JSON.parse(java.ajax("https://d.lhdqsb.cn/source/"+parseInt(java.getString('$.data.book_id')/1000)+"/"+java.getString('$.data.book_id')+".html,"+JSON.stringify(option))).data[0].site_path
"https://catalog.lhdqsb.cn/"+pt
