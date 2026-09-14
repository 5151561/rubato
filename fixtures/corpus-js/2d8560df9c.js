// from: 飞速中文 .ruleToc.nextTocUrl
var res = java.get('real_chapter');
    options = org.jsoup.Jsoup.parse(res).select('option');
    for (var i in options){
        options[i] = options[i].attr('value')
    }
    options
