// from: library .ruleSearch.intro
{{eval(source.bookSourceComment)}}

`
出版社:{{$.publisher}}
ISBN:{{$.isbn}}
出版时间:{{$.year}} 
文件格式:{{$.extension}} 
文件大小:${getFileSize({{$.filesize}})}
`
