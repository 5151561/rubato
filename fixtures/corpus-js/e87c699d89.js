// from: 🌐 淘小说网 .ruleSearch.bookUrl
q= "https://tybook.taoyuewenhua.net/tf/book?";
w="&key=mibook_123456";
o='{{$.sourceId}}';
t="appid=mibook&bid="+o+"&brand=realme&optype=1&ostype=0&osversion=7.1.2&t=1655577023334&version_code=273";
i=t+w;
y="&sign=";
u=java.md5Encode(i).toUpperCase();
q+t+y+String(u)
