// from: 晋江文学① .ruleToc.chapterList
last=JSON.parse(java.ajax('http://android.jjwxc.net/androidapi/chapterList?novelId='+java.get('id'))).chapterlist;
last=last[last.length-1];
list=JSON.parse(result).chapterlist;
list.push(last);
JSON.stringify(list)
