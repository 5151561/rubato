// from: 飞速中文 .ruleContent.content
function aq(ar, as) {
    var aK = ar['length'];
    var aL = (ar[(aK-0x1)]&0xffffffff);
    for (var aM = 0x0; (aM<aK); aM++) {
        ar[aM] = String['fromCharCode'](ar[aM] & 0xff, ((ar[aM]>>>0x8)&0xff), ((ar[aM]>>>0x10)&0xff),(ar[aM]>>>0x18) & 0xff);
    }
    if (as) {
        return ar['join']('')['substring'](0x0, aL);
    } else {
        return ar['join']('');
    }
}
function aQ(aR, aS) {
    var b6 = aR.length;
    var b8 = [];
    for (var b7 = 0x0; b7< b6; b7 += 0x4) {
        b8[b7 >> 0x2] = ((aR['charCodeAt'](b7)| aR['charCodeAt']((b7+0x1)) << 0x8) | (aR['charCodeAt']((b7+0x2))<< 0x10)|(aR['charCodeAt'](b7 + 0x3)<<0x18));
    }
    if (aS) {
        b8[b8['length']] = b6;
    }
    return b8;
}
function c6(c7, c8) {
    if (c7==='') {
        return '';
    }
    var cS = aQ(c7, false);
    var cX = aQ(c8, false);
    var cT = cS['length'] - 0x1;
    var cP = cS[cT - 0x1],
    cQ = cS[0x0],
    cR = 0x9e3779b9;
    var cL, cM, cN = Math['floor'](0x6+(0x34/(cT+0x1))),
    cO = (cN*cR) & 0xffffffff;

    while (cO!= 0x0) {
        cM = (cO >>> 0x2) & 0x3;
        for (var cW = cT; cW > 0x0; cW--) {
            cP = cS[cW-0x1];
            cL = (((cP>>>0x5)^cQ << 0x2)+((cQ>>>0x3)^(cP<<0x4))) ^ (cO^cQ) + (cX[((cW&0x3)^cM)]^cP);
            cQ = cS[cW] = ((cS[cW] - cL)&0xffffffff);
        }
        cP = cS[cT];
        cL = ((((cP>>>0x5)^(cQ<<0x2)) + (cQ >>> 0x3 ^ (cP<<0x4)))^((cO^cQ) + (cX[((cW&0x3)^cM)]^cP)));
        cQ = cS[0x0] = cS[0x0] - cL & 0xffffffff;
        cO = (cO-cR) & 0xffffffff;
    }
    return aq(cS, true);
}
function base64decode(str) {
	var code = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".split("");
    var bitString = "";
    var tail = 0;
    for (var i = 0; i < str.length; i++) {
      if (str[i] != "=") {
        var decode = code.indexOf(str[i]).toString(2);
        bitString += (new Array(7 - decode.length)).join("0") + decode;
      } else {
        tail++;
      }
    }
    Bin = bitString.substr(0, bitString.length - tail * 2);
    var result = "";
    for (var i = 0; i < Bin.length; i += 8) {
      result += String.fromCharCode(parseInt(Bin.substr(i, 8), 2));
    }
    return result;
  }

var res = (function (_src){
    if (_src.indexOf('uid_name')>-1){
        java.log('★页面跳转★');
        var data_json = _src.match(/var data = (\{(.*\s*)*?\})/)[1].replace(/\'/g,'\"');
        var data = JSON.parse(data_json);
        cookie.setCookie('https://m.feibzw.com',data['upi_name']+ "=" +escape(data['upi_value'])+";"+data['uid_name']+ "=" +escape(data['uid_value'])+";");
        return java.ajax(data['url']);
    }else{
        java.log('☆页面正常☆');
        return _src;
    }
})(src);
var content=res.match(/Content='(.*?)'/)[1];
var update=res.match(/update='(.*?)'/)[1];
text = decodeURIComponent(escape(base64decode(c6(base64decode(content),update))));
text?text:org.jsoup.Jsoup.parse(res).select('#nr1').html()
