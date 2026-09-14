// from: 📚 精品书城 .searchUrl
eval(String(source.bookSourceComment));


var key = "　" + key;
var b64 = java.base64Encode(key);
var b32 = base32(String(b64));


var option = {
    "method": "POST",
    "body": "keywords=" + encodeURIComponent(key) + "&base64="+ b64 + "&base32=" + b32 + "&rndval="+new Date().getTime()
}

"https://jingpinshucheng.com/index.php?c=book&a=sojsonp," + JSON.stringify(option);
