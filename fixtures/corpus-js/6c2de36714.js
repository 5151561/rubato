// from: 猴子阅读 .ruleBookInfo.init
eval(String(source.bookSourceComment));

$ = JSON.parse(result).data;
if($.Name.match(/{{}\}/)){
		$.Name = decode($.Name);
	}
if($.Author.match(/{{}\}/)){
		$.Author = decode($.Author);
	}
JSON.stringify($);
