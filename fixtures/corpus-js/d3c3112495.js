// from: 💯阅友 .searchUrl
time=Math.round(new Date()/1000);

uth=
java.HMacHex("b313789b-2d2a-41ce-8982-26af5271fe7c&"+time,"HmacMD5","snY%169j");
    	
tmpToken=java.randomUUID().toString().replace("-", "").toLowerCase();
java.put("tmpToken",tmpToken);

sign=
java.HMacHex("/goway/goread/app/search/getBookByKeywordplatId=2&appId=com.yueyou.adreader&channelId=pc&appVersion=3.6.4&srcChannelId=pc&time="+time+"&_s=C66A42978B576873D32F0EACEBBF044D&utId=996ff65deb7fbfc9f9bdcefb6c62f4ce&deviceId=183860518871512&userId=y76498329&sex=boy&wx=0&tmpToken="+tmpToken+"&st=2&uth="+uth+"&keyword="+key+"&page=1&psize=20&withRecommend=1","HmacSHA256","snY%169j");

_p=java.desEncodeToBase64String("utId=996ff65deb7fbfc9f9bdcefb6c62f4ce&deviceId=183860518871512&userId=y76498329&sex=boy&wx=0&tmpToken="+tmpToken+"&st=2&uth="+uth+"&sign="+sign,"snY%169j","DES/ECB/PKCS5Padding","");

let option={
"method": "POST",
"body": "psize=20&page=1&withRecommend=1&keyword={{key}}"};

"/goway/goread/app/search/getBookByKeyword?platId=2&appId=com.yueyou.adreader&channelId=pc&appVersion=3.6.4&srcChannelId=pc&time="+time+"&_s=C66A42978B576873D32F0EACEBBF044D&_p="+java.encodeURI(_p)+","+JSON.stringify(option)
