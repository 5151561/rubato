// from: 笔趣阁a .ruleToc.chapterList
//目录
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
let json = JSON.parse([result].join('').replace(/},]/g, '}]'));
let chapters = [];
json.data.list.forEach(group => {
    group.list.forEach(chapter => {
        if (chapter.hasContent == 1) {
            chapters.push(
                {
                    title: decrypt(chapter.name),
                    url: baseUrl.replace('infosxs.pysmei.com', 'contentxs.pysmei.com').replace('index.html', chapter.id + '.html')
                }
            );
        }
    });
});
chapters;
