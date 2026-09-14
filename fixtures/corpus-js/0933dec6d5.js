// from: 🔰笔趣阁.pysmei .ruleContent.content
a=String(chapter.title).replace(/\s/g,"");
result.substring(0,90).includes(a)?result=result.split(a,2)[1]:result;
result.toLowerCase()
