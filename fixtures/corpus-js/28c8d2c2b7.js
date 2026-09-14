// from: 💠 海警学院 .ruleExplore.bookList
a=[];b="0123456789abcdef"
for(i=0;i<6;i++){for(I in b){a.push(i+b[I])}}
a.push("60","61","62","63")
b="的一是了我不人在他有这个上们来到时大地为子中你说生国年着就那和要她出也得里后自以会家可下而过天去能对小多然于心学么之都好看起发当没成只如事把还用第样道想作种开美总从无情己面最女但现前些所同日手又行意动"
for(i in b){
		reg=new RegExp('<i>&#xe8'+a[i]+';</i>','gi')
		result=result.replace(reg,b[i])
	}
result
