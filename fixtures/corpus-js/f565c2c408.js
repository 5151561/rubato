// from: ♛ 连城读书 #渊呀1107 .searchUrl
/*isGet=true:获取token;isGet=false:不获取token;*/
/*想获取改为true,并填上对应的账号密码，获取成功后改为false*/
isGet=false
if(isGet){
resp=JSON.parse(java.ajax('http://h5.lc1001.com/h5/login,{"method":"POST","body":"pcd=账号&pwd=密码"}'))
java.log('请复制 UID: '+resp.uID)
java.log('请复制 token: '+resp.token)}
time=Math.round(new Date())
url="POSThttp://a.lc1001.com/app/query/keybooksconsumerKey=LCREAD_ANDROIDpn="+(page-1)*20+"timestamp="+time+"uID=0XKrqBSeeEwgDy2pT"
"http://a.lc1001.com/app/query/keybooks?consumerKey=LCREAD_ANDROID&timestamp="+time+"&sign="+java.md5Encode(encodeURIComponent(url))+"&uID=0&deviceID=0&kw="+key+"&pn="+(page-1)*20
