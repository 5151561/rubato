// from: 国漫吧 .ruleContent.content
header={"Referer":baseUrl};
headers={"headers":JSON.stringify(header)};
imgl=eval(result.match(/(eval\(.+?\}\)\))/)[1]);

host = "http://images.720rs.com";
image=cInfo.fs;
//piclist.map(u=>"<img src=\""+server+u+','+JSON.stringify(headers)+"\">").join("\n")

html='';
function get7ImageUrl(image) {
    if (image.match(/^(\/Man?)/i)) {
        return host + image
    } else if (image.match(/^(http?)/i)) {
        return image
    }
}
for(i in image){
url=get7ImageUrl(image[i]);
html+='<img src="'+url+','+JSON.stringify(headers)+'">\n'
}
html
