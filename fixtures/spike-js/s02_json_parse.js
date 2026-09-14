// expect: 剑来,雪中悍刀行
JSON.parse(result).data.list.map(function(b){return b.name;}).join(',');
