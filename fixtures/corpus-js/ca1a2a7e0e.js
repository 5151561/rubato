// from: 新浪小说网移动端 .ruleToc.nextTocUrl
list=[]
total_page=String(java.get('total_page'))
for(var i = 2;i<=total_page;i++){
list.push('http://book.sina.cn/dpool/newbook/bookv1/ajax/get_catalog.php?bid=@get:{bid}&page_size=50&page='+i)
}
list
