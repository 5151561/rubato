// from: 酷安应用评论 .ruleContent.content
if(!/apk\/detail/.test(baseUrl)){
    let id = "{{$.data.id}}";
    let html = ""
    let item = JSON.parse(result).data;
    let curPage = 1;
    let lastItem = "";
    let resp2 = JSON.parse(java.ajax("https://api.coolapk.com/v6/feed/replyList?id={{$.data.id}}&page=1&lastItem="));
    let replynum5 = item.replynum;
    replynum5 > 100?replynum5=100:replynum5
    html += createReply(item)
    let item_reply = []
    let totalnum = 0
    let item2 = resp2.data
    item_reply = item_reply.concat(item2)
    while (item2.length && (totalnum = totalnum + item2.length) < replynum5) {
      curPage = curPage + 1
      lastItem = item2[item2.length - 1].id
      resp2 = JSON.parse(java.ajax(`https://api.coolapk.com/v6/feed/replyList?id=${id}&page=${curPage}&lastItem=${lastItem}`))
      item2 = resp2.data
      item_reply = item_reply.concat(item2)
    }

    let grouped_item_reply = {}
    item_reply.map(item => {
      if (item.rrid) {
        grouped_item_reply[item.rrid] = grouped_item_reply[item.rrid] || []
        grouped_item_reply[item.rrid].push(item)
      } else {
        grouped_item_reply[item.id] = grouped_item_reply[item.id] || []
        grouped_item_reply[item.id].push(item)
      }
    })
    for (key in grouped_item_reply) {
      grouped_item_reply[key].sort((e1, e2) => {
        return e1.dateline - e2.dateline
      }).map(item => html += createReply(item, !!item.rrid))
    }
    result = String(html).replace(/<a .*?href="([^"]+)".*?>([^<]+)<\/a>/g,'▪$2：$1▪')
   }else{
   	let html = "软件地址：\nhttps://www.coolapk.com/apk/{{$.data.apkname}}\n";
   	 html +="---复制下面文字到发现规则可看评论---\n"
   	html += "{{$.data.title}}▪回复::https://api.coolapk.com/v6/page/dataList?url=#/feed/apkCommentList?id={{$.data.id}}&sort=&title=最近回复&page={\{page}}&lastItem=\n"

html += "{{$.data.title}}▪发布::https://api.coolapk.com/v6/page/dataList?url=#/feed/apkCommentList?id=256030&sort=dateline_desc&title=最近发布&page={\{page}}&lastItem=\n"

html += "{{$.data.title}}▪热度::https://api.coolapk.com/v6/page/dataList?url=#/feed/apkCommentList?id={{$.data.id}}&sort=popular&title=热度排序&page={\{page}}&lastItem=\n"
   	}
