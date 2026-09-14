// from: 🔖花生小说 .ruleToc.chapterUrl
n=baseUrl.match(/book_id=(\d+)/)[1];
result='https://api.wan123x.com/read/getChapterDetail?book_id='+n+'&chapter_id={{$.chapter_id}}'
