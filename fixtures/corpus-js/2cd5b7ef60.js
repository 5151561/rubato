// from: 笔墨看书网 .ruleContent.content
let json = java.ajax("https://" + result.match(/q.bimoks.com\/data.*?\.xs/)[0])
        if (json != null) {
            json = eval("(" + json.replace("_txt_call(", "").replace(")", "") + ")")
            for (let key in json.replace) {
                let i = new RegExp(json.replace[key], "ig");
                json.content = json.content.replace(i, key)
            }
            result = json.content
        }
