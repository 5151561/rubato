// from: 艾途小说 .ruleContent.content
url="https:"+result.match(/initTxt\("([^"]+)","[^"]+"\)/)[1];
r=String(java.ajax(url).match(/_txt_call\(([\s\S]+\})\)/)[1]);
var r = eval('(' + r + ')');
				if(r.content!=null){				
      var e = r.replace;
				for (var n in e) {
					var i = new RegExp(e[n], "ig");
					r.content = r.content.replace(i, n)
				}
				result=r.content
					}else{result="章节加载失败，或者内容正在手打中，请【收藏本站】稍访问或者联系管理员更新~"}
