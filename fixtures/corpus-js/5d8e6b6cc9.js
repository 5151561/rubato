// from: 废文网 .ruleContent.content
function d(a,b){
	b=java.md5Encode(b);
	var d=b.substring(0,16);
	var e=b.substring(16);
	return java.aesBase64DecodeToString(a,e,"AES/CBC/PKCS5Padding",d)
}
eval(result.match(/(d\("[\s\S]+"\))\)/)[1])
