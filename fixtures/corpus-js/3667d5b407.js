// from: 📚 有度中文 .ruleToc.chapterUrl
if(result.indexOf("☆")!=-1){cid=parseInt(result.match(/\d+(?=☆)/)[0]);
nex=cid+1;pre=cid-1;
result=result.replace(/(\d+)☆☆/,pre).replace(/(\d+)☆/,nex)}
result
