// from: 🌐 兔九三网 .ruleContent.nextContentUrl
if (result.indexOf("xyy2.png") > -1) {
var code = result.match(/eval\(function(.*)\);/)[0]
code = code.replace(/__u[0-9a-f]+/, '__uData')
eval(code)
	
	if(!__uData.match('_')) {
	 next = java.getElement("@@#linkNext@a")
	 __uData = next.attr("href")
	}
__uData
}
