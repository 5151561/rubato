// from: 📥 百度贴吧 .ruleContent.content
String(result).replace(/\s*该楼层疑似违[\s\S]+?查看此楼|<img.*?class="nicknameEmoji".*?>/g,'').replace(/\s*<span class="tail-info">\s*/g,'—').replace(/<li class="d_name">[\s\S]+?ad-dom-img="true">/g,'').replace(/<img.*?ad-dom-img="true">/g,'').replace(/<img class="icon-jubao".*?>|—来自.*?端|手机贴吧|.*?快来下载吧！.*?>/g,'')
