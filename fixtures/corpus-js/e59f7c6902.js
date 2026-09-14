// from: 💠 笔趣阁⁵²⁰ .ruleToc.nextTocUrl
page=result.match(/\(第\d+\/(\d+)页\)当前\d+条\/页/)[1];
list=[];for(i=2;i<=page;i++){list.push(baseUrl.replace(/_1/,'_'+i))}
list
