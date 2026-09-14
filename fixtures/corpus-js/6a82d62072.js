// from: 笔趣阁a .ruleBookInfo.init
//详情
function decrypt(data) {
    try {
        let mark = decodeURI('%7B%7B%7B%7D%7D%7D');
        if (data && data.indexOf(mark) > -1) {
            let part = data.split(mark)[1];
            let decrypted = java.createSymmetricCrypto('desede/CBC/PKCS5Padding', 'OW84U8Eerdb99rtsTXWSILDO', 'SK8bncVu').decryptStr(part);
            if (decrypted) {
                return decrypted;
            }
        }
    } catch (e) {
    }
    return data;
}
let json = JSON.parse(result);
let data = json.data;
let info = {
    title: decrypt(data.Name),
    cover: 'https://imgapixs.pysmei.com/bookfiles/bookimages/' + decrypt(data.Img),
    desc: decrypt(data.Desc),
    author: decrypt(data.Author),
    category: decrypt(data.CName),
    status: decrypt(data.BookStatus),
    update: decrypt(data.LastChapter),
    updateTime: decrypt(data.LastTime),
    tocUrl: baseUrl.replace('info.html', 'index.html')
};
info;
