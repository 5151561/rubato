// from: 📚 熊猫看书 .ruleContent.nextContentUrl
a=baseUrl.match(/pg=(\d+)/)[1];
a&&a<{{$.result.pagecount}}?baseUrl.replace(/pg=.+/,'pg='+(Number(a)+1)):''
