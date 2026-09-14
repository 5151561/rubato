// from: ♛淘小说  .ruleToc.chapterUrl
time=Date.now();
bid=java.getString("$.bid");
cid=java.getString("$.cid");
sign=java.md5Encode("appid=mibook&bid="+bid+"&brand=HUAWEI&channel=Tencent&cid="+cid+"&device_id=22f71e7a68a04e8a8e6618ace8804404&model=vmos&optype=1&ostype=0&osversion=7.1.2&package_name=com.martian.ttbook&t="+time+"&token=54338ca6-a8eb-4662-9ae4-ae1f871b0825&uid=45090208&version_code=266&version_name=8.1.9&key=mibook_123456");

"https://tybook.taoyuewenhua.net/tf/chapter_content?appid=mibook&bid="+bid+"&brand=HUAWEI&channel=Tencent&cid="+cid+"&device_id=22f71e7a68a04e8a8e6618ace8804404&model=vmos&optype=1&ostype=0&osversion=7.1.2&package_name=com.martian.ttbook&t="+time+"&token=54338ca6-a8eb-4662-9ae4-ae1f871b0825&uid=45090208&version_code=266&version_name=8.1.9&sign="+sign;
