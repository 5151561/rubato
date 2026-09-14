// from: 拂袖 .ruleToc.chapterUrl
function sign(list){
    var sb='2bb6ffe20b1eff00';
    list=list.sort();
    for(var i=0;i<list.length;i++){if(i==0){sb+=list[i];}else{sb+="&";sb+=list[i];}}
    sb+="cabf8a3adca20e831d7e96267f709032";
    return String(java.md5Encode(sb)).toUpperCase();
}
time=Math.round(new Date()/1000);
list=["book_id={{$.book_id}}","chapter_id={{$.chapter_id}}","packageName=com.fuxiuyuedu.fuzReader","time="+time,"token="+java.get('token'),"marketChannel=vivo","appId=","sysVer=7.1.2","osType=2","udid=1f705dac-2eec-3045-96be-713264df32c2","ver=1.0.0","product=1"];
body={"sign":sign(list),"book_id":{{$.book_id}},"chapter_id":{{$.chapter_id}},"packageName":"com.fuxiuyuedu.fuzReader","time":time,"token":String(java.get('token')),"marketChannel":"vivo","appId":"","sysVer":"7.1.2","osType":"2","udid":"1f705dac-2eec-3045-96be-713264df32c2","ver":"1.0.0","product":"1"};
option={"method":"POST","body":JSON.stringify(body)};
"/chapter/text," + JSON.stringify(option);
