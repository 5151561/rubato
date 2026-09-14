// from: 豆腐app .ruleBookInfo.lastChapter
String(result).replace(/<i class="icon icon-lock-on fr"><\/i>/g,'🔒').replace(/<li class="list_item">/g,'').replace(/<a href="[^"]+">\s*/g,'').replace(/<\/a>/g,'').replace(/<\/li>/g,'').replace(/^\s/,'')
