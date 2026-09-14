// from: 奇妙中文[api] .ruleExplore.bookUrl
bid = String(java.getElements("a").attr("href")).match(/shuku\/(.*)\//)[1];//提取出来数据
java.put("bid",bid);//目录页代码替换，让目录页调用此参数
"https://api-miao.qimao.com/api/book/chapter-list?book_id=" + bid
