// from: 酷安应用评论 .jsLib
function pad(s) {
  return s < 10 ? `0${s}` : s
}

function formatDate(timeStamp){
	let diff = (Date.now() - timeStamp * 1000) / 1000
    if (diff < 60) {
      return '刚刚'
    } else if (diff < 3600) {
      return `${parseInt(diff / 60)}分钟前`
    } else if (diff < 86400) {
      return `${parseInt(diff / 3600)}小时前`
    } else if (diff < 604800) {
      return `${parseInt(diff / 86400)}天前`
    } else if (diff < 2592000) {
      return `${parseInt(diff / 604800)}周前`
    } else if (diff < 31104000) {
      return `${parseInt(diff / 2592000)}个月前`
    } else {
      let date = new Date(timeStamp * 1000)
      return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
    }
	}
	
function createReply(item, addPadding) {
	  let html = "";
    addPadding?fu = "▲":fu=""
    html += `<br>&lrm;<br>---${fu}${item.username}▪${formatDate(item.dateline)}---<br>`
    html += `${item.message}`
    if (item.picArr && item.picArr.length) {
      item.picArr.filter(it => it).map(it => {
        html += `<img src="${it}"><br>`
      })
    } else if (item.pic) {
      html += `<img src="${item.pic}"><br>`
    }
    return html
  }
