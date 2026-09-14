// from: 💗 值得阅读 .header
(()=>{
    var ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 7_1_2 like Mac OS X) AppleWebKit/537.51.2 (KHTML, like Gecko) Version/7.0 Mobile/11D167 Safari/9537.53";
    var time=Math.round(new Date()/1000);
    var sign=java.md5Encode("com.ruffianhankin.meritreader1"+time+"vhjJVz1St6tK7!8n#B0MqRIuE2Dh7!C#");
    var pt = "1";
    var Content="text/html; charset=UTF-8";
    var Connection="close";
    var Accept="*/*";
    var Origin="*";
    var Headers="X-Requested-With";
    var Vary="Accept-Encoding";
    var package="com.ruffianhankin.meritreader";
    var version="3.8.6";
    var channel="baidu_tg104";
    var Cache="no-cache, no-store";
    var headers = {"Content-Type":Content,"Connection":Connection,"Accept":Accept,"Access-Control-Allow-Origin":Origin,"Access-Control-Allow-Headers":Headers,"Vary":Vary,"User-Agent":ua,"package":package,"pt":pt,"version":version,"channel":channel,"time":String(time),"sign":String(sign),"Cache-Control":Cache};
    return JSON.stringify(headers);
})()
