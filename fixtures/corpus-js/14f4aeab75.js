// from: 衍墨轩~ymxxs .ruleContent.content
a=String(java.get("tt")).replace(/\s|[（【][一二三四五六七八九十上中下终续]{1,2}[】）]/g,"");
result.substring(0,90).includes(a)?result=result.split(a,2)[1]:result;
result.toUpperCase()
