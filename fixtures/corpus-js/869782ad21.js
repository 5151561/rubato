// from: 💠 海警学院 .ruleBookInfo.init
url = java.getString('text.全部章节@href||#listtj@a.0@href');
html = url!=""?(baseUrl='https://www.hjgzf.com'+url,java.ajax(baseUrl)):result
java.put('tocUrl', baseUrl)
html
