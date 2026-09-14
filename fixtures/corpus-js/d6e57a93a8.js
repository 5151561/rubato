// from: 梦幻小说 .ruleToc.chapterList
//获取目录，根据div的data-id排序
list=java.getElements("@@id.listsss@div").toArray().sort((a,b)=>a.attr("data-id")-b.attr("data-id"));

//创建数组
l=[];


for(i in list){
a=list[i].select("a").toArray();
l=l.concat(a)
}

l.map(x=>x)
