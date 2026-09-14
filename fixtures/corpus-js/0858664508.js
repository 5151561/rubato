// from: 💯阅友 .ruleBookInfo.tocUrl
time=Math.round(new Date()/1000);

uth=
java.HMacHex("b313789b-2d2a-41ce-8982-26af5271fe7c&"+time,"HmacMD5","snY%169j");

sign=
java.HMacHex("/userCenter/getChapterListbookId={{$.id}}&chapterId=0&isAll=1&platId=2&appId=com.yueyou.adreader&channelId=pc&appVersion=3.6.4&srcChannelId=pc&time="+time+"&_s=C66A42978B576873D32F0EACEBBF044D&utId=996ff65deb7fbfc9f9bdcefb6c62f4ce&deviceId=183860518871512&userId=y76498329&sex=boy&wx=0&tmpToken="+java.get("tmpToken")+"&st=1&uth="+uth,"HmacSHA256","snY%169j");

_p=java.desEncodeToBase64String("utId=996ff65deb7fbfc9f9bdcefb6c62f4ce&deviceId=183860518871512&userId=y76498329&sex=boy&wx=0&tmpToken="+java.get("tmpToken")+"&st=1&uth="+uth+"&sign="+sign,"snY%169j","DES/ECB/PKCS5Padding","");

let option={
"method": "POST",
"body":"tmp="};

"http://dl.reader.yueyouxs.com/userCenter/getChapterList?bookId={{$.id}}&chapterId=0&isAll=1&platId=2&appId=com.yueyou.adreader&channelId=pc&appVersion=3.6.4&srcChannelId=pc&time="+time+"&_s=C66A42978B576873D32F0EACEBBF044D&_p="+java.encodeURI(_p)+","+JSON.stringify(option)
