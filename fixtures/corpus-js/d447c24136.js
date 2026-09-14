// from: 📥 免费小说 .ruleContent.content
content=java.getString(".plus_l li:not(:has(span))@html",false)+'<br> ━ 长按选择浏览器打开下载 ━ <br>'+java.getElements(".dl_kkbd_ccc a")
.toArray()
.map(a => '【' + a.text() + '】http://www.freexiaoshuo.com' + a.attr('href'))
.join('\n');
