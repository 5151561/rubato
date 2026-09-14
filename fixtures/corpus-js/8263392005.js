// from: 面包FM .ruleExplore.bookUrl
body = {"book_id":{{$.book_id}},"apptoken":"","user_id":"0"}
option = {"method":"POST","body": JSON.stringify(body)}
"/api/fmapp_bookinfo," + JSON.stringify(option)
