// from: 🌟笔趣阁~pysmei .ruleContent.content
//判断正文是否加密
pos=result.indexOf("\{\{\{\}\}\}");
pos==-1?result:result=java.aesBase64DecodeToString(result.slice(pos+6),"OW84U8Eerdb99rtsTXWSILDO","DESede/CBC/PKCS5Padding","SK8bncVu");
//错漏章过滤
/^[\s\S]{0,50}?(?:@font-face|\{\"code\":|此章节正在|防盗章|字数填充)|(?:正在手打中|努力更新中|内容更新后)\W+请[重稍在]|为防止内容获取不全和文字乱序|网站已经不更新最新章节|网站此章内容错误|…。。\r\s+！\f\s+$|\n\s+[\(“]\f\s+$|\u0000{9}\s+$/.exec(result)?'':result
