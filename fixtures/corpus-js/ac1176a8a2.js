// from: 福书网 .searchUrl
(()=>{
  if(page==1){
    let url=source.getKey()+'/e/search/index.php,'+JSON.stringify({
    "method":"POST",
    "body":`keyboard=${key}&show=title&tempid=1&tbname=article`
    });
    return java.put('surl',String(java.connect(url).raw().request().url()));
  } else {
    return java.get('surl').replace(/result.+searchid/,`result/index.php?page=${page-1}&searchid`);
  }
})()
