// from: 一读 .ruleSearch.bookList
if(result.match(/^<script/)&&result.match(/\/script>$/)){
	java.log('get Cookie...')
	$=java.head(baseUrl,{referer:baseUrl}).cookies();
	source.putLoginHeader(JSON.stringify({
		'Cookie': 't='+$.token+';r='+($.secret-100)
	}));
	url=baseUrl+`,{'headers':{'referer':'${baseUrl}'}}`
	result=java.ajax(url)
}
result
