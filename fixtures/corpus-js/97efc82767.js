// from: 无错小说网 .ruleToc.nextTocUrl
page=result.match(/第\d+页 \/ 共(\d+)页/)[1];
list=[];for(i=2;i<=page;i++){list.push(baseUrl.replace(/\/1/,'/'+i))}
list
