// from: 提花小说网 .ruleContent.content
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
					}else{result="章节加载失败，或者内容正在手打中，请稍后访问~"}
