// from: 🌸大米小说 .ruleContent.nextContentUrl
(()=>{
  	  let res = Array.from(result).join("")
  	  let url = "";
  	  if(res){
  	  	 // java.log(res)
  	  	  let id = baseUrl.match(/_(\d)\.html/)
  	  	  if(id){
  	  	    url = baseUrl.replace(/\d\.html/,(+id[1]+1)+".html")
  	  	  }else{
  	  	  	  url = baseUrl.replace(/\.html/,"_2.html")
  	  	  	}
  	  	}
  	  //	java.log(url)
  	  	return url
  	})()
