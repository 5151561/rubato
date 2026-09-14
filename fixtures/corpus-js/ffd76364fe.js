// from: 断秋风 .ruleToc.nextTocUrl
if(/\/1$/.test(baseUrl)){
	var url=baseUrl
	}else{
		var url=baseUrl+"1"
		}
		var surl=String(url)
y=java.getString("@@#pagestats@text")
var page=y.match(/\d+\/(\d+)/)[1]
list=[];for(i=2;i<=page;i++){
	list.push(surl.replace(/\/1$/,"/"+i))
	}
	list
