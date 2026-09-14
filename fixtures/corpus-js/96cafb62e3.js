// from: 飞速中文 .ruleToc.chapterList
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
    java.put('real_chapter',res);
    org.jsoup.Jsoup.parse(res).select('.chapter li a');
