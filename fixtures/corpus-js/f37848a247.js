// from: 🔖书枫迟诗 .searchUrl
time=Math.round(new Date()/1000)
sign=java.md5Encode("com.diandianbook.androidreading1"+time+"vhjJVz1St6tK7!8n#B0MqRIuE2Dh7!C#")
headers={"package":"com.diandianbook.androidreading","pt":"1","time":String(time),"sign":String(sign)}
option={"headers":headers}
"https://s.fjwhcbsh.com/v1/lists.api?keyword={{key}},"+JSON.stringify(option)
