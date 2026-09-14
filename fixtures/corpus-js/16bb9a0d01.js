// from: 阅友小说 .ruleContent.content
u0 = "/userCenter/v310/downloadChapter";
u3 = "bookId="+java.get("bookid")+"&chapterId="+java.get("chapterId");


url = "https://dl.reader.yueyouxs.com"+
eval(String(source.bookSourceComment));

java.ajax(JSON.parse(java.ajax(url)).data.contentUrl);
