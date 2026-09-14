// from: 圣武书库[阅读] .header
(()=>{
	var ua = "navigator.userAgent.toLowerCase(); 			return { 				'mobile': !!(ua.match(/applewebkit.*mobile.*/) || ua.match(/iemobile/) || ua.match(/windows phone/) || ua.match(/android/) || ua.match(/iphone/) || ua.match(/ipad/)), 				'weixin': ua.indexOf('micromessenger') > -1 			};";
	var heders = {"User-Agent": ua};
	return JSON.stringify(heders);
})()
