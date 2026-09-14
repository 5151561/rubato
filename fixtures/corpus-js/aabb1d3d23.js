// from: 来看文学 .searchUrl
try{
bUrl=baseUrl.replace(/\/?\#\#.*/,'');
cookie.removeCookie(bUrl)
s=String(java.ajax(bUrl))
act=s.match(/"act".*?"(.{16})"/)[1]
sub=s.match(/name="submit".*?value="(.*?)"/)[1]
body=String("act="+act+"&q="+key+"&submit="+sub)
xx=body.match(/[^&=]+/g)
for(i in xx){
re=java.encodeURI(xx[i],"gbk")
body=body.replace(xx[i],re)
 }
var url=String(bUrl+"/sscc/,"+JSON.stringify({
	"method":"POST","body":body}))
java.put("key",body.match(/q=([^&]*)/)[1]);
java.put("url",bUrl+"/sscc/"+java.post(bUrl+"/sscc/",body,{}).header("Location"));url
}catch{java.longToast("获取失败")}
