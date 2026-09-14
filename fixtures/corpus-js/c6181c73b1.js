// from: 💊 115文学 .ruleContent.nextContentUrl
if (result.indexOf("next.png") > -1) {
	
	code = result.match(/eval\(function(.*)\);/)[0]
	code = code.replace(/u[0-9a-f]+/, 'uData')
	eval(code)
		
	if(!uData.match('_')) {
	 next = java.getElement("@@class.ud-link ud-link1@span@a")
	 uData = next.attr("href")
	}
	uData
}
