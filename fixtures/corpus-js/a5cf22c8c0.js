// from: 🔞 优特 .ruleToc.chapterList
list=java.getElements("//*[@class=\"mread\"][1]/tbody//tr[position()>1]");
p=""
for(i in list){
	java.setContent(list[i]);
	a=java.getElement("@@class.bai");
	b=java.getElement("@@class.du");
	p+=String(a)+String(b)
	}
p
