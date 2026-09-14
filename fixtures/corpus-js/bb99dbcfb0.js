// from: 🔖读书阁① .ruleToc.nextTocUrl
let count = java.get('total');
let list = [];
for(let i = 2; i <= Number(count); i++) {
list.push(baseUrl.replace(/page=\d+/, 'page=' + i));
}
list;
