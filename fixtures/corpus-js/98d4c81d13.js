// from: 🎉 西瓜小说 .ruleExplore.bookUrl
var javaImport = new JavaImporter();
javaImport.importPackage(
    Packages.java.lang,
    Packages.javax.crypto,
    Packages.javax.crypto.spec,
    Packages.java.security,
    Packages.java.security.interfaces,
    Packages.java.security.spec,
    Packages.java.io,
    Packages.java.util
);
with(javaImport){
    function encrypt(str){
        var bArr = String(str).getBytes("UTF-8");
        var mac = Mac.getInstance("HmacSHA256");
        mac .init(SecretKeySpec(String("wj3imab73kwceuf51lf01ORHe2cmo8X0YrZwF4p2uv3WEfmqxrT2oIBwRFRNErXW20UKal15ZTDdxPKU43puZFqcuXkvrQmadhp1wn6YPEDO4WRgInp8NNNQo4uNsWF1CELOzx7yPOS4pQbhTRWB4qRm0a4ENIegN0SH7K2STbsyaCuWh7m7s3rTpb5dK3CcDdsT35vo0xNbPZI2dJmKoIeKk9p3YhBLNWp9WoZqn9Qihpvn4nvgJzVSacgy2MTo").getBytes("UTF-8"),"HmacSHA256"));
        return mac.doFinal(bArr)
    }
  function encode(bArr) {
        if (bArr === null) {
            return null;
        }
        var length = bArr.length * 8;
        if (length === 0) {
            return "";
        }
        var i10 = length % 24;
        var i11 = parseInt(length / 24);
        var cArr = new Array((i10 !== 0 ? i11 + 1 : i11) * 4);
        var i12 = 0;
        var i13 = 0;
        var i14 = 0;
        while (i12 < i11) {
                var i15 = i13 + 1;
                var b10 = bArr[i13];
                var i16 = i15 + 1;
                var b11 = bArr[i15];
                var i17 = i16 + 1;
                var b12 = bArr[i16];
                var b13 = (b11 & 15)&255;
                var b14 = (b10 & 3)&255;
                var i18 = b10 & -128;
                var i19 = b10 >> 2;
                if (i18 !== 0) {
                    i19 ^= 192;
                }
                var b15 = i19 & 255;
                var i20 = b11 & -128;
                var i21 = b11 >> 4;
                if (i20 !== 0) {
                    i21 ^= 240;
                }
                var b16 = (i21 & 255);
                var i22 = (b12 & -128) === 0 ? b12 >> 6 : (b12 >> 6) ^ 252;
                var i23 = i14 + 1;
                var cArr2 = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '+', '/'];
                cArr[i14] = cArr2[b15];
                var i24 = i23 + 1;
                cArr[i23] = cArr2[(b14 << 4) | b16];
                var i25 = i24 + 1;
                cArr[i24] = cArr2[(b13 << 2) | (i22&255)];
                cArr[i25] = cArr2[b12 & 63];
                i12++;
                i14 = i25 + 1;
                i13 = i17;
				}
        if (i10 === 8) {
            var b17 = bArr[i13];
            var b18 = b17 & 3;
            var i26 = b17 & -128;
            var i27 = b17 >> 2;
            if (i26 !== 0) {
                i27 ^= 192;
            }
            var i28 = i14 + 1;
            var cArr3 = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '+', '/'];
            cArr[i14] = cArr3[i27];
            var i29 = i28 + 1;
            cArr[i28] = cArr3[b18 << 4];
            cArr[i29] = '=';
            cArr[i29 + 1] = '=';
		}else if (i10 === 16) {
            var b19 = bArr[i13];
            var b20 = bArr[i13 + 1];
            var b21 = (b20 & 15)&255;
            var b22 = (b19 & 3)&255;
            var i30 = b19 & -128;
            var i31 = b19 >> 2;
            if (i30 !== 0) {
                i31 ^= 192;
            }
            var b23 = i31&255;
            var i32 = b20 & -128;
            var i33 = b20 >> 4;
            if (i32 !== 0) {
                i33 ^= 240;
            }
            var i34 = i14 + 1;
            var cArr4 = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '+', '/'];
            cArr[i14] = cArr4[b23];
            var i35 = i34 + 1;
            cArr[i34] = cArr4[(i33&255) | (b22 << 4)];
            cArr[i35] = cArr4[b21 << 2];
            cArr[i35 + 1] = '=';
        }
        return cArr.join('');
    }
}

var bookId= java.getString("bookId");

var myDate = new Date();
var timestamp = myDate.getFullYear() + 
"0" + (myDate.getMonth()+1) + 
"0" + myDate.getDate() + 
"0" + myDate.getDay() + 
myDate.getMinutes() + 
myDate.getSeconds();

var sign = encode(encrypt("appId=100398183&country=CN&lang=zh_CN&ver=10009248&appVer=1.0.9.248&timestamp="+ timestamp +"{\"bookId\":\""+bookId+"\"}8cfaaedef7a7e24a716732ff5428958f"))

url = "https://xgmf.zuanqianyi.com/glory/free/1110?appId=100398183&country=CN&lang=zh_CN&ver=10009248&appVer=1.0.9.248&timestamp=" + timestamp + ",";
post={
  "method": "POST",
  "headers": {
    "pname": "com.dzmf.zmfxsdq",
    "sign": sign,
    "signType": "1"
  },
  "body":'{"bookId":"'+bookId+'"}'
}
url+JSON.stringify(post)
