// from: 🐳猫耳 .ruleBookInfo.tocUrl
href = java.getString("@@class.drama-name@href");
if(baseUrl.match(/dramaapi|mdrama/) || href!=""){
id = baseUrl.match(/drama_id=(\d+)/)?baseUrl.match(/(\d+)/)[1]:href.match(/(\d+)/)[1]
result='https://www.missevan.com/dramaapi/getdrama?drama_id='+id
}else if(baseUrl.match(/player/)){
	result='https://www.missevan.com/sound/getsound?soundid='+baseUrl.match(/(\d+)/)[1]
	}
