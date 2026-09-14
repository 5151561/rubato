// from: 🌐 淘小说网 .ruleToc.chapterUrl
w="&key=mibook_123456";
o='{{$.bid}}';
a='{{$.cid}}';
t="appid=mibook&bid="+o+"&brand=realme&cid="+a+"&optype=1&ostype=0&osversion=7.1.2&package_name=com.martian.ttbook&t=1655577023334&uid=52533755&version_code=273&version_name=8.5.2";
i=t+w;
y="&sign=";
u=java.md5Encode(i).toUpperCase();
"https://tybook.taoyuewenhua.net/tf/chapter_content?"+t+y+String(u)
