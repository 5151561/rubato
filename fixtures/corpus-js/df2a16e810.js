// from: 🔖 百度知道 .ruleContent.content
result=String(result).replace(/<.*?wgt-replyer-all-time">([^<]+)<\/span>/g,'<h1 class="wgt-replyer-all-time">---$1---</h1>').replace(/<span.*?>\d+<\/span>/g,'').replace(/<\/*span.*?>/g,'').replace(/展开全部/g,'')
