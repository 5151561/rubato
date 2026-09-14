// from: 📝 书仓网🔖 .searchUrl
let headers={
  "Referer":"https://m.shucangxs.com/",
  "User-Agent":"Mozilla/5.0 (Linux; Android 11; MI CC9 Pro Build/RKQ1.200826.002; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/92.0.4515.131 Mobile Safari/537.36"
};

(()=>{
  let base='https://m.shucangxs.com';
  if(page==1){
    let url=base+'/search.html';
    let body='searchkey='+key;
   
    return base+java.put('surl',java.post(url,body,headers).header("Location"));
  } else {
    return base+java.get('surl');
  }
})()
