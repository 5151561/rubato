// from: 深夜看书 .ruleContent.content
try {
	// Aes解密
	matchResult = src.match(/html\(d\(['"](.+)['"],\s+['"](.+)['"]\)\);/);
	string = matchResult[1];
	code = java.md5Encode(matchResult[2]);
	iv = code.substring(0, 16);
	key = code.substring(16);
	result = java.createSymmetricCrypto("AES/CBC/PKCS7Padding", key, iv).decryptStr(string);
} catch (err) {
	//解密失败使用webView加载重新获取内容
	src = java.ajax(baseUrl + ',{"webView": true}');
	result = java.getString('#C0NTENT@html', src);
}
result
