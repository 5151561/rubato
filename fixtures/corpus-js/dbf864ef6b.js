// from: 📥 书荒部落 .ruleContent.content
content=java.getString(".plus_l li:not(:has(span))@html",false)+'<br> ━ 长按选择浏览器打开下载 ━ <br>'+java.getElements(".list a")
.toArray()
.map(a => '【' + a.text() + '】' + a.attr('href'))
.join('\n');
