// from: 101言情 .ruleBookInfo.init
if(result.match(/text=new Array\(".*?"\)/)){
con=""
eval(result.match(/text=new Array\(".*?"\)/)[0]);
for(var i=0;i<text.length;i++){con +=text[i];}con=String(java.base64Decode(con));
result=con
}else{
	result=result
	}
