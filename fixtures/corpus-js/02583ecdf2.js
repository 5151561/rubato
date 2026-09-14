// from: 刊 .ruleBookInfo.intro
intro=book.intro?book.intro:"";
text="{{@@class.time_source_subline2@tag.a@text||class.til@text}}"
href="{{@@class.time_source_subline2@tag.a@href||text.浏览往期@href##history.*}}history_{\{Number(String(java.timeFormat(Date.now())).replace(/\\/.*/,''))-(page-1)}\}.html"

rule=text?"\n---复制下面的文字粘贴到发现，可以浏览该期刊往期---\n"+text+"::"+href:""
intro+rule
