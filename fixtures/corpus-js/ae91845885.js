// from: 耽美文库 .searchUrl
(()=>{
  if(page==1){
    let url=source.getKey()+'/search/index.php,'+JSON.stringify({
  "body": `keyboard=${key}&show=title&tempid=2&tbname=article`,
  "charset": "GBK",
  "method": "POST"
});
    return java.put('surl',String(java.connect(url).raw().request().url()));
  } else {
    return java.get('surl')+'&page='+(page-1)
  }
})()
