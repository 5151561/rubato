// from: 晋江文学① .ruleSearch.bookList
key=java.get('key');
page=java.get('page');

//创建两个空数组
json=[];json2=[];

//判定json里是否有items
if(JSON.parse(result).items){
json=JSON.parse(result).items;}

//加载作者搜索页面并转为json
json1=JSON.parse(java.ajax('http://android.jjwxc.net/androidapi/search?keyword='+key+'&type=2&page='+page+'&searchType=7&sortMode=DESC'));

//判定json里是否有items
if(json1.items){
json2=json1.items
}

//书名搜索列表与作者搜索列表拼接
list=json.concat(json2);


result=JSON.stringify(list)
