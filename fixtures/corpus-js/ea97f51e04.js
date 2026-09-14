// from: 🎉 闪舞小说 .searchUrl
url = "https://www.35xss.com/search.html";

body = "searchtype=all&searchkey="+key;

head = {
	"referer": "https://www.35xss.com/",
	"cookie": "0"
};

$ = java.post(url,body,head).headers();

url = $.Location||$.location;

String(url).replace('1.html','{{page}\}.html');
