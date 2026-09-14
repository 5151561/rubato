// from: 亲亲小说手机版 .ruleBookInfo.init
var o=(selector,b)=>{
return String(java.getString("@@"+selector,b));};
var book={name:o(".title@text",false),
author:o(".zuozhe@text",false),
kind:o(".synopsisArea_detail@tag.p.1:2:3@text",false),
lastChapter:o(".directoryArea@tag.a.0@text",false),
intro:o(".review@text",false),
coverUrl:o("div@img@src",true),
tocUrl:o("text.查看完整目录@href",true)
};
book;
