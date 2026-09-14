// from: 🔖 百度百科 .ruleContent.content
String(org.jsoup.Jsoup.parse(result).select('h2,.reference-title,.para,.reference-list,.basic-info')).replace(/<h2 class="block-title">目录<\/h2>/,'').replace(/<h2.*?>/g,'-----📖').replace(/<\/h2>/g,'📖-----').replace(/<span class="title-prefix">.*?span>/g,'').replace(/<dt class="basicInfo-item name">/g,'---------')
