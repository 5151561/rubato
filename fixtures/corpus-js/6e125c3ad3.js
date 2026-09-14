// from: 起点中文 .ruleSearch.bookList
path='class.res-book-item';
u=java.get('url');
c=java.getElement(path);
if (!c.length && result.includes('var buid')) {
  cookie.removeCookie(source.getKey());
  java.toast('在网页加载完成后，手动点击右上角 [√] 即可');
  java.startBrowserAwait('https://www.qidian.com/all/','过起点搜索的 cookie 验证');
  a=java.ajax(u);
  java.setContent(a);
  java.getElement(path);
}
