// from: 和图书 .ruleContent.content
if(result.match(/^<!DOCTYPE html><html lang="en-US"><head><title>Just a moment...</)){java.longToast('请根据网页提示点击勾选「确认您是真人」来通过人机验证，如果无限循环请查看源注释说明。');result=java.startBrowserAwait(baseUrl,'人机验证').body();};result;
loadDocType="{{@@body@data-doctype}}";
loadType="{{@@body@data-randomtype}}";
sid=baseUrl.match(/(\d+).html/)[1];
url=baseUrl.replace(/\d+.html/,'')

var section = {
    loadDocType: loadDocType,
    loadType: loadType,
    sid:sid,
    content: {
        childNode: [],
        init: function(c){
            this.body=c
            if (section.loadType == 'normal') {
      this.load(String(java.base64Decode(java.getString('//*[@name="client"]/@content'))))
            } else if (section.loadType == 'substep') {
                var d = 'r' + section.sid;
                var f = false;
                if (section.loadDocType == 'xml') {
                    d += '.xml'
                } else if (section.loadDocType == 'json') {
                    d += '.json';
                    f = true
                } else {
                    return false
                };        
             cookie='PHPSESSID='+java.get(baseUrl,{'User-Agent':'Mozilla/5.0 (Linux; Android 12; Nexus 5X Build/NRD90M); wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/115.0.4664.104 Mobile Safari/537.36'}).cookie('PHPSESSID');
            token= java.get(url+d,{'X-Requested-With': 'XMLHttpRequest','referer':baseUrl,'cookie':String(cookie),'User-Agent':'Mozilla/5.0 (Linux; Android 12; Nexus 5X Build/NRD90M); wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/115.0.4664.104 Mobile Safari/537.36'}).header('token')              
                	this.load(token)      	
            } else {
                return false
            };               
        },
        load: function(a) {         
            a = String(java.base64Decode(a)).split(/[A-Z]+%/);      
            var b = 0,
            start = 0;
            for (var i = 0; i < this.body.length; i++) {
                if (String(this.body[i]).match(/h2/i)) {
                    start = i + 1
                };
                if (String(this.body[i]).match(/div/i)&& this.body[i].attr("class") != 'chapter') {
                    break
                }
            };
            for (var i = 0; i < a.length; i++) {
                if (a[i] < 5) {
                    this.childNode[a[i]] = this.body[i + start];
                      b++
                } else {
                    this.childNode[a[i] - b] = this.body[i + start]                 
                }
            };
  }
}
}
section.content.init(java.getElement("@@id.content@children!0").toArray());
section.content.childNode.join("")
