// from: 🐳猫耳 .ruleSearch.bookList
key=java.get('key');
page=java.get('page');

//创建两个空数组
json=[];json2=[];

//判定json里是否有列表
if(JSON.parse(result).info.Datas){
json=JSON.parse(result).info.Datas;}

//加载单曲搜索页面并转为json
json1=JSON.parse(java.ajax('https://www.missevan.com/sound/getsearch?s='+key+'&type=3&page_size=10&p='+page));

//判定json里是否有列表
if(json1.info.Datas){
json2=json1.info.Datas
}

//剧集搜索列表与单曲搜索列表拼接
list=json.concat(json2);

result=JSON.stringify(list)
