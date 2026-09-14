// from: 🌐 就去看网 .ruleBookInfo.coverUrl
(s=java.getStringList('@css:img[alt*='+book.name+']@src||meta[property$=image]@content||img[src~=(cover|file|article)[^a-z]|/\\d+[/_-]\\d+(s?\\.|$)]@src||img[data-src~=\\S]@data-src||img[src*=/img]@src||img[src~=^(data|https?):|^[^:]+/]@src')).size()?/^data:/.test(s=s.get(0))?java.base64Encode(s):s:null
