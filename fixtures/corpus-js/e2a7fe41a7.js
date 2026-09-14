// from: 📚 期刊杂志 .ruleSearch.bookList
//书名修饰
for(i in result){
	var t='《'+result[i].title+'》 - ';
	//java.log(t);
	result[i].span=result[i].span.replace(/^/,t);
}
result
