// from: 模板（Json） .ruleBookInfo.init
var J = org.jsoup.Jsoup.parse(result);
var o = selector => String(J.select(selector).text()) ;
var og = selector => String(J.select('[property="og:' + selector + '"]').attr('content'));
var book = {
	name: og('novel:book_name').replace(/(全文|小说|免费阅读|最新章节).*|[(（].*[）)]/g, ''),
	author: og('novel:author'),
	kind: og('novel:category') + ',' + og('novel:status'),
	latest: og('novel:latest_chapter_name').replace(/^(正文|VIP章节|最新章节)?(\s+|_)|[\(（【].*[求更谢乐发推].*/g, '') + '▪' + og('novel:update_time').replace(/(T|\s).*/, ' ').replace(/\//g, '-'),
		intro: og('description').replace(/.*(观看小说|简介)[:：]|各位书友.*/g, '').replace(/\s+/g, ''),
	cover: og('image'),
	url: og('novel:read_url'),
};
book;
