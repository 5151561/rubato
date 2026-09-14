// from: 【优品PPT】 .ruleBookInfo.intro
var lis = {
  比例:(String(src).match(/比例：.*?\n/)),
  页数:(String(src).match(/页数：.*?\n/)),
  格式:(String(src).match(/格式：.*?\n/)),
  大小:(String(src).match(/大小：.*?\n/)),
  日期:(String(src).match(/日期：.*?\n/)),
  效果:(String(src).match(/效果：.*?\n/))
}
"    " + lis['比例'] + lis['页数'] + lis['格式'] + lis['大小'] + lis['日期'] + lis['效果']
