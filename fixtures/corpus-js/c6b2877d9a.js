// from: 👔 疯情书库 .ruleBookInfo.init
eval(String(source.bookSourceComment))
var J = org.jsoup.Jsoup.parse(result),
  url = baseUrl.replace(/book-/, "chapterList-")
  book = {
    name: String(J.select(".book_name a").text())
      .replace(/(全文|小说|免费阅读|最新章节).*|[\(（].*[）\)]/g, ''),
    author: J.select(".author").text(),
    kind: String(J.select(".xinxi_content span").text().split(" ")[1])
      .replace(/[中已]/g, "")
      + "," +
      J.select(".xinxi_content span").text().split(" ")[0]
      + "," +
      J.select(".dt_r").text().split(" ")[0],
    word: String(J.select(".xinxi_content span").text().split(" ")[2])
      .replace(/字/, ""),
    latest: String(J.select(".new_zhangjie1 .dt_l a").text())
      .replace(/^(正文|VIP章节|最新章节)?(\s+|_)/g, "")
      .replace(/[\(（【].*?[求更谢乐发订合贺补加推票章字修防kK].*/g, "")
      + "·" +
      String(J.select(".new_zhangjie1 .dt_r").text()).replace(/\s.*/, "")
      + "·\[" +
      String(org.jsoup.Jsoup.parse(java.ajax(url)).select(".section_list li"))
        .split("\n").length
      + "\]",
    intro: "<br>" + String(J.select(".jianjieneirong").html()).replace(/\s/g,"<br>"),
    cover: host + "/" + J.select(".book_info_top_l img").attr("src"),
    url: url
  };
//java.log(JSON.stringify(book))
book;
