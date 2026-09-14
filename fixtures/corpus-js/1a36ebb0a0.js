// from: 🌾色彩夏书 .ruleContent.content
!(/google.cn/).test(baseUrl)?
java.getString("$..content"):
decodeURIComponent(baseUrl.replace(/^.*?text=/, ''))
