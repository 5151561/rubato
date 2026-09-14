// from: 来看文学 .exploreUrl
sort=[];
 push=(title,url,type)=>sort.push({
		title: title,
		url: url,
		style: {
				layout_flexGrow: 1,
				layout_flexBasisPercent: type
			}
	});
	u=source.getKey();
	url=u+"/menu.html";
	D=org.jsoup.Jsoup.parse(java.ajax(url));
	A=D.select('div[class="divmen"] a');
	for(i in A){
		if(i!=0&&i!=4&&i!=5){
			push(A[i].text(),A[i].attr("href"),"0.25")
		}
	}
	JSON.stringify(sort);
