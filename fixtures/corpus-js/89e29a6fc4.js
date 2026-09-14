// from: 笔趣阁a .ruleSearch.bookList
//搜索结果列表
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
function getResult() {
    return json.data.map(function (item) {
        let id = Number.parseInt(decrypt(item.Id + ''));
        return {
            cover: decrypt(item.Img),
            url: 'https://infosxs.pysmei.com/BookFiles/Html/' + (Math.floor(id / 1000) + 1) + '/' + item.Id + '/info.html',
            title: decrypt(item.Name),
            author: decrypt(item.Author),
            category: decrypt(item.CName),
            update: decrypt(item.LastChapter),
            updateTime: decrypt(item.UpdateTime),
            desc: decrypt(item.Desc)
        }
    })
}
getResult();
