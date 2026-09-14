// from: 酷安应用评论 .header
let TOKEN = 'token://com.coolapk.market/c67ef5943784d09750dcfbb31020f0ab?'
let PACKAGE_NAME = 'com.coolapk.market';
let DEVICE_ID = String(java.randomUUID());
let APP_DEVICE = (length => {
  let chars = '0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ'
  let result = ''
  for (var i = length; i > 0; --i) {
    result += chars[Math.floor(Math.random() * chars.length)]
  }
  return result
})(50);

  let t = parseInt(Date.now() / 1000)
  let hex_t = `0x${t.toString(16)}`
  let md5_t = java.digestHex(t.toString(), "md5")
  let a = `${TOKEN}${md5_t}$${DEVICE_ID}&${PACKAGE_NAME}`;
  let md5_a = java.digestHex(String(java.base64Encode(a)),"md5")
  let token = `${md5_a}${DEVICE_ID}${hex_t}`

head = {
      'User-Agent': 'Dalvik/2.1.0 (Linux; U; Android 10; Pixel 4 XL Build/QQ2A.200405.005) +CookMarket/10.1.2-2004302',
      'X-Requested-With': 'XMLHttpRequest',
      'X-Sdk-Int': '29',
      'X-Sdk-Locale': 'zh-CN',
      'X-App-Id': 'com.coolapk.market',
      'X-App-Version': '10.1.2',
      'X-App-Code': '2004302',
      'X-App-Token': token,
      'X-Api-Version': '10',
      'X-App-Device': APP_DEVICE,
      'X-Dark-Mode': '0'
    }
JSON.stringify(head)
