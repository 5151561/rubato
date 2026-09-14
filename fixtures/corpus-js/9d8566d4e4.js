// from: 🔖 百度知道 .ruleToc.nextTocUrl
url=baseUrl.match(/(.*?pn=)/)[1];
list=[];
for(i=2;i<=10;i++){
list.push(url+(i*10))
}
list
