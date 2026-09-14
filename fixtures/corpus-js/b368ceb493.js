// from: 🌐 淘小说网 .searchUrl
q= "https://tybook.taoyuewenhua.net/authopt/ty/search_books?";
w="&key=mibook_123456";
e=key;
r=page-1;
t="appid=mibook&brand=HONOR&channel=HuaWei&ctype=1&device_id=f659e11471f94e9d861c58a061c9d60f&keywords="+e+"&model=PCT-AL10&optype=1&ostype=0&osversion=7.1.2&package_name=com.martian.ttbook&page="+r+"&pageSize=10&searchType=1&sourceName=yw&t=1667399501553&token=0923c6b9-0681-4472-b9db-1a88071299b5&uid=62793078&version_code=395&version_name=8.9.8";
i=t+w;
y="&sign=";
u=java.md5Encode(i).toUpperCase();
q+t+y+String(u)
