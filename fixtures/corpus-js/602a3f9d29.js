// from: 🔖书枫迟诗 .ruleSearch.lastChapter
time=Math.round(new Date()/1000)
sign=java.md5Encode("com.diandianbook.androidreading1"+time+"vhjJVz1St6tK7!8n#B0MqRIuE2Dh7!C#")
headers={"package":"com.diandianbook.androidreading","pt":"1","time":String(time),"sign":String(sign)}
option={"headers":headers}
resp=JSON.parse(java.ajax("https://book.fjwhcbsh.com/details/"+parseInt(java.getString('$.book_id')/1000)+"/"+java.getString('$.book_id')+".html,"+JSON.stringify(option))).data;resp.last_chapter_name+" • "+resp.updated_at
