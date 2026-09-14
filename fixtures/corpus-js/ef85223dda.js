// from: 番茄小说2 .ruleContent.content
let cid = java.hexDecodeToString(result);
eval(String(source.loginUrl));
let genreValue = JSON.parse(java.ajax(book.bookUrl)).data[0].genre;
if (genreValue === '4') {
  option = '&tone_id=0';
}
java.get(source.bookSourceUrl+'/content?item_id='+cid+option+'&key='+Map('密钥'),{"Content-Type": "application/json","Accept":"application/json, text/plain, */*"}).body()
