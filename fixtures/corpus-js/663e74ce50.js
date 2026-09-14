// from: 趣书网/qubook .searchUrl
(()=>{
  if(page==1){
    let url=source.getKey()+'/e/search/index.php,'+JSON.stringify({
    "method":"POST",
    "body":"show=title&btzz=b&keyboard="+key
    });
    return java.put('surl',String(java.connect(url).raw().request().url()));
  } else {
  	java.log('surl')
    return java.get('surl')+'&page='+(page-1)
  }
})()
