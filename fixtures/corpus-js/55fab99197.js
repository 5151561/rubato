// from: 得间小说[apiq] .ruleToc.nextTocUrl
totalPage=JSON.parse(result).body.pageInfo.totalPage
url=[]
for(i=1;i<=totalPage;i++)url.push(baseUrl+'?page='+i)
url
