// from: 🐳猫耳 .ruleSearch.bookUrl
id=String(result).match(/id=(\d+)/)[1];
if(!String(result).match(/catalog_name/)){
result='https://www.missevan.com/sound/getsound?soundid='+id
}else{result='https://www.missevan.com/dramaapi/getdrama?drama_id='+id}
