// from: 玄幻文学 .ruleContent.nextContentUrl
if (result.indexOf("nex22.png") > -1) {
  var code = result.match(/eval\(function(.*)\);/)[0];
  code = code.replace(/u[0-9a-f]{10,}/, 'uData')
  var uData = "";
  eval(code);
	
	if(!uData.match('_')) {
	next = java.getElement("@@#pb_next@a")
	uData = next.attr("href")
	}
	
  uData;
}
