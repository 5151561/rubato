// from: 🎨 爱优漫📱  .ruleContent.content
header={"Referer":baseUrl};
headers={"headers":JSON.stringify(header)};

//获取章节位置
index=parseInt(chapter.index);
index=index+1;
num=parseInt(book.totalChapterNum);
index=num-index;

//画质可选low middle high
var img_ext = "-aym.middle.webp";
var pic = "https://mhpic."
var html ="";

json=JSON.parse(result);
comic_chapter= json.data.comic_chapter;

var end_num  = comic_chapter[index].end_num;
var rule=comic_chapter[index].rule;
chapter_domain=comic_chapter[index].chapter_domain;

for( let i = 1;i <= end_num; i++) {
   let url = pic + chapter_domain +rule.replace(/\$\$/g,i) +img_ext;
   html += '<img src="' + url +','+JSON.stringify(headers)+'">\n';
}
result = html
