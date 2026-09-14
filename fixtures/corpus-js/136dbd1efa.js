// from: ♛ 连城读书 #渊呀1107 .ruleToc.chapterUrl
/*获取到的 uid 和 token 分别填入*/
uID="0"
token="0"
time=Math.round(new Date())
isvc={{$.ISVC}}
if(isvc==0){
u="/app/book/pubchapter"
}else{
u="/app/book/vipchapter"
}
url="GEThttp://a.lc1001.com"+u+"bID="+java.get('bid')+"cID={{$.CID}}consumerKey=LCREAD_ANDROIDtimestamp="+time+"uID="+uID+"XKrqBSeeEwgDy2pT"
u+"?consumerKey=LCREAD_ANDROID&timestamp="+time+"&sign="+java.md5Encode(encodeURIComponent(url))+"&bID="+java.get('bid')+"&cID={{$.CID}}&uID="+uID+"&token="+token+"&mType=OPPO-PCRT00&PACKINGCHANNEL=YINGYONGBAO"
