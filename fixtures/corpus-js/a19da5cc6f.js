// from: 🔖星空小说网 .ruleContent.content
java.getElements('@@id.txt@dd').toArray().sort((a,b)=>a.attr('data-id')-b.attr('data-id')).map(x=>x.html()).join('')
