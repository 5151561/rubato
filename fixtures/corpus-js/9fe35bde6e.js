// from: 番茄小说2 .ruleToc.chapterList
eval(String(source.loginUrl));
function deviceType() {
  try {
    return!!java.androidId();
  } catch (e) {
    return false;
  }
}
let device = deviceType()? 'android' : 'ios';
let genreValue = JSON.parse(java.ajax(book.bookUrl)).data[0].genre;
//如果 genre 值为 4，书籍类型赋值为 听书函数
if (genreValue === '4') {
  option = '&tone_id=0';
}
// 根据不同条件为 book.type 赋值
if (device === 'android') {
  if (option) {
    // 如果 option 有内容，安卓阅读赋值为 32
    book.type = 32;
  } else if (genreValue === '1') {
    // 如果 genre 值为 1，安卓阅读赋值为 64
    book.type = 64;
  } else {
    // 默认情况下，安卓阅读赋值为 8
    book.type = 8;
  }
} else if (device === 'ios') {
  if (option) {
    // 如果 option 有内容，苹果源阅赋值为 1
    book.type = 1;
  } else if (genreValue === '1') {
    // 如果 genre 值为 1，苹果源阅赋值为 2
    book.type = 2;
  } else {
    // 默认情况下，苹果源阅赋值为 0
    book.type = 0;
  }
}

function spArr(arr, num) {
  let newArr = []
  for (let i = 0; i < arr.length;) {
    newArr.push(arr.slice(i, i += num))
  }
  return newArr
}
let res = JSON.parse(result).data
let item_list = spArr(res["allItemIds"], 100)
let array = []
for (let i = 0; i < item_list.length; i ++) {
  let response = java.ajax('https://novel.snssdk.com/api/novel/book/directory/detail/v1/?item_ids='+item_list[i])
  let data = JSON.parse(response).data
  data.forEach((x) => {
    array.push({
      name: x.title,
      url: `data:;base64,${java.base64Encode(x.item_id)},{"type":"pyfqc"}`,
      info: ((Number(x.volume_name) == 0?"第一卷：默认": x.volume_name)+' · '+java.timeFormat(x.first_pass_time*1000)+' · '+x.chapter_word_number+'字')
    })
  })
}
array
