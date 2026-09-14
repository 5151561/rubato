// from: ⚡️读书阁 .searchUrl
url='http://www.zmtt.net/checkUpdate,{"method":"POST","body":{"version":"2.0"}}'
url=JSON.parse(java.ajax(url)).data.url

'http://'+url+'search,{"method":"POST","body":{"keyword":"{{key}}"}}'
