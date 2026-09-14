// from: 📖💯读书阁🔖🅰 .exploreUrl
option={"method":"POST","body":{"version":"2.0"}}
url="http://"+JSON.parse(java.ajax("http://www.zmtt.net/checkUpdate,"+JSON.stringify(option))).data.url

list=[{"title":"全部小说","url":"rankList?type=全部&page={{page}}&state=0","style":{"layout_flexGrow":0.25}},{"title":"都市小说","url":"rankList?type=都市&page={{page}}&state=0","style":{"layout_flexGrow":0.25}},{"title":"科幻小说","url":"rankList?type=科幻&page={{page}}&state=0","style":{"layout_flexGrow":0.25}},{"title":"女频小说","url":"rankList?type=女频&page={{page}}&state=0","style":{"layout_flexGrow":0.25}},{"title":"历史小说","url":"rankList?type=历史&page={{page}}&state=0","style":{"layout_flexGrow":0.25}},{"title":"网游小说","url":"rankList?type=网游&page={{page}}&state=0","style":{"layout_flexGrow":0.25}},{"title":"玄幻小说","url":"rankList?type=玄幻&page={{page}}&state=0","style":{"layout_flexGrow":0.25}},{"title":"其他小说","url":"rankList?type=其他&page={{page}}&state=0","style":{"layout_flexGrow":0.25}},{"title":"修真小说","url":"rankList?type=修真&page={{page}}&state=0","style":{"layout_flexGrow":0.25}}]

list.forEach(x=>{
	x.url=url+x.url
	})
JSON.stringify(list)
