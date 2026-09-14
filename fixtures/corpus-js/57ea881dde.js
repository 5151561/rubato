// from: 起点中文网💰 .ruleBookInfo.tocUrl
var bid = baseUrl.match(/info\/(\d+)/)[1];
java.put('bid', bid);
'https://intldruid.qidianglobal.com/Atom.axd/Api/Book/GetChapterList?BookId=' + bid+'&timeStamp=0&requestSource=0&md5Signature=';
