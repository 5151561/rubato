// from: 🎈牛站小说「PO文」 .ruleContent.content
content = {  
    star: 0,   
    childNode: [],
    childNodes: java.getElements("@@id.novelcontent@p").toArray(),
   UpWz: function(m, i) {
        var k = Math.ceil((i + 1) % this.code);
        k = Math.ceil(m - k);
        return k
      },
    load: function() {
    this.code=result.match(/code=(\d+)/)[1];
    var e = String(java.base64Decode(java.getString('//*[@name="client"]/@content'))).split(/[A-Z]+%/);
    var j = 0; 
    for (var i = 0; i < e.length; i++) {
    var k = this.UpWz(e[i], i);
    this.childNode[k] = this.childNodes[i]
    };
    }
}
if(baseUrl.match(/\d+_2/)){
content.load();
result=content.childNode.join("<br>");}else{result=java.getElements("@@id.novelcontent@p")}
