// from: 七猫小说 .ruleExplore.bookList
gender=baseUrl.match(/gender=(\d+)/)?baseUrl.match(/gender=(\d+)/)[1]:""
category_id=baseUrl.match(/category_id=(\d+)/)?baseUrl.match(/category_id=(\d+)/)[1]:""
need_filters=baseUrl.match(/need_filters=(\d+)/)?baseUrl.match(/need_filters=(\d+)/)[1]:""
page=baseUrl.match(/page=(\d+)/)?baseUrl.match(/page=(\d+)/)[1]:""
need_category=baseUrl.match(/need_category=(\d+)/)?baseUrl.match(/need_category=(\d+)/)[1]:""
tag_id=baseUrl.match(/tag_id=(\d+)/)?baseUrl.match(/tag_id=(\d+)/)[1]:""
sign_key='d3dGiJc651gSQ8w1'
headers={'app-version':'51110','platform':'android','reg':'0','AUTHORIZATION':'','application-id':'com.****.reader','net-env':'1','channel':'unknown','qm-params':''}
headers['sign']=String(java.md5Encode(Object.keys(headers).sort().reduce((pre,n)=>pre+n+'='+headers[n],'')+sign_key))


var urlEncode = function (param, key, encode) {
  if(param==null) return '';
  var paramStr = '';
  var t = typeof (param);
  if (t == 'string' || t == 'number' || t == 'boolean') {
    paramStr += '&' + key + '=' + ((encode==null||encode) ? encodeURIComponent(param) : param);
  } else {
    for (var i in param) {
      var k = key == null ? i : key + (param instanceof Array ? '[' + i + ']' : '.' + i);
      paramStr += urlEncode(param[i], k, encode);
    }
  }
  return paramStr;
};

var category = function () {
  params={'gender':gender,'category_id':category_id,'need_filters':need_filters,'page':page,'need_category':need_category}
  params['sign']=String(java.md5Encode(Object.keys(params).sort().reduce((pre,n)=>pre+n+'='+params[n],'')+sign_key))
  url="https://api-bc.wtzw.com/api/v4/category/get-list?"+urlEncode(params)
  return java.ajax(url+','+java.put("headers",JSON.stringify({"headers":headers})))
};

var tag = function () {
  params={'gender':gender,'need_filters':need_filters,'page':page,'tag_id':tag_id}
  params['sign']=String(java.md5Encode(Object.keys(params).sort().reduce((pre,n)=>pre+n+'='+params[n],'')+sign_key))
  url="https://api-bc.wtzw.com/api/v4/tag/index?"+urlEncode(params)
  return java.ajax(url+','+java.put("headers",JSON.stringify({"headers":headers})))
};


if(baseUrl.match(/category/)){
  category()
}else {
  tag()
}
