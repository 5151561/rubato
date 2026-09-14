// from: 💰长河阅读[api] .ruleContent.content
JSON.parse(result).data.show_content.reduce((pre,n)=>pre+java.base64Decode(n.content)+'\n','')
