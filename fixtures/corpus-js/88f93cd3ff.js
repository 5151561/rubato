// from: 📚 期刊杂志 .ruleSearch.bookList
//java.log(result);
var list=[];
var option={
'charset': 'UTF-8',
'headers': {
    'User-Agent': 'Mozilla/5.0 (Linux; Android 7.0; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/48.0.2564.116 Mobile Safari/537.36 T7/10.3 SearchCraft/2.6.2 (baiduboxapp; P1 7.0)'
    }
};

//获取期刊列表
for(i in result){
	var str='http://new-qk.lifves.com/' +String(result[i])+','+JSON.stringify(option);
	list.push(str);
}
java.log(list.length);
var res=[];
var num=4;
var len=list.length;

//分割并获取每一期刊内部刊数列表
for(i=0;i<len;i+=num){
	var ls=list.splice(0,num);
	java.log(list.length+','+ls.length);
	var temp=java.ajaxAll(ls);
	res.push(temp);
}
java.log('请求结束'+res.length);

//将列表整合成json
var json='[';
for(i in res){
	//java.log(i);
	for(j in res[i]){
	 var html=res[i][j].body();
	 var json1=html.match(/JSON.parse\('([^']+)'/)[1];
	 //java.log(json1);
	 json+=json1+',';
	}
}
json=String(json).slice(0,-1)+']';
JSON.parse(json);
