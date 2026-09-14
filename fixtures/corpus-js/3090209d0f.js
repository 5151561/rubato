// from: 七猫小说 .ruleExplore.bookUrl
sign_key='d3dGiJc651gSQ8w1'

params={'id':{{$.id}},'imei_ip':'2937357107','teeny_mode':0}

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
paramSign=String(java.md5Encode(Object.keys(params).sort().reduce((pre,n)=>pre+n+'='+params[n],'')+sign_key))
params['sign']=paramSign
"https://api-bc.wtzw.com/api/v4/book/detail?"+urlEncode(params)+","+java.get("headers")
