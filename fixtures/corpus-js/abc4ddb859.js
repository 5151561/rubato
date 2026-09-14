// from: 蛋文库 .searchUrl
(()=>{
  if(page==1){
    let url=source.getKey()+'/e/search/index.php,'+JSON.stringify({
    "method":"POST",
    "charset": "GB2312",
    "body":"show=title,newstext&keyboard="+key
    });
    return java.put('surl',String(java.connect(url).raw().request().url()));
  } else {
    return java.get('surl').replace(/result.+searchid/,"result/index.php?page="+page+"&searchid")
  }
})()
