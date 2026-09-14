// from: 书香之家 .header
(()=>{
    var ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 6_1_3 like Mac OS X) AppleWebKit/536.26 (KHTML, like Gecko) Version/6.0 Mobile/10B329 Safari/8536.25_com.shuxiangzhijia.xyz";
    var time=Math.round(new Date()/1000);
    var sign=java.md5Encode("com.shuxiangzhijia.xyz1"+time+"vhjJVz1St6tK7!8n#B0MqRIuE2Dh7!C#");
    var pt = "1";
    var Content="text/html; charset=UTF-8";
    var Connection="close";
    var Accept="*/*";
    var Origin="*";
    var Headers="X-Requested-With";
    var Vary="Accept-Encoding";
    var package="com.shuxiangzhijia.xyz";
    var version="1.0.3";
    var channel="kuaishou_tg101";
    var Cache="no-cache, no-store";
    var headers = {"Content-Type":Content,"Connection":Connection,"Accept":Accept,"Access-Control-Allow-Origin":Origin,"Access-Control-Allow-Headers":Headers,"Vary":Vary,"User-Agent":ua,"package":package,"pt":pt,"version":version,"channel":channel,"time":String(time),"sign":String(sign),"Cache-Control":Cache};
    return JSON.stringify(headers);
})()
