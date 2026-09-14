// from: 📚 有度中文 .ruleToc.chapterList
result.replace(/(\/book\/.+?)(\.html" class=.+?\n.+? href=")(javascript:)(?=" class=)/g,"$1$2$1☆.html").replace(/(javascript:)(" class=.+?\n.+? href=")(\/book\/.+?)(?=\.html" class=)/g,"$3☆☆.html$2$3")
