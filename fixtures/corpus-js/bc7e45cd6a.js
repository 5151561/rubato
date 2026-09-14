// from: 来看文学 .ruleSearch.bookList
eval(String(source.bookSourceComment))
try{
	actyzm=result.match(/actyzm" value="([^"]+)"/)[1];
	button=result.match(/button" value="([^"]+)"/)[1];	
	url=java.get("url");
	code=String(url).replace(/\/sscc.*/,'')+"/member/getcode.asp";
	yzco=source.getVariable();
	function getOption(str,co){
  	hed={"Referer":url,"Cookie":co}
  	return JSON.stringify({"method":"POST",
"charset":"gb2312","body":"actyzm="+actyzm+"&yzm="+str+"&button="+button,"headers":hed});
  }
  function succes(){
		yzm=java.getVerificationCode(code);
		url+=","+getOption(yzm,"");
		hm=java.ajax(url);
		co=java.getCookie(url,"g3jy9sqo0ws");
   source.setVariable(yzm+',g3jy9sqo0ws='+co)
   return hm;
  }
	if(yzco&&yzco!=''){
		v=String(yzco).split(',');
		u=url+","+getOption(v[0],v[1]);
		result=java.ajax(u);
		if(/请输入验证码/.test(result)){
			java.longToast("  ！验证码过期或有误，重新获取  ！")
			result=succes();
		}
	}else{result=succes("");}
result
}catch(err){java.log(err);result}
