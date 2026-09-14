// from: 👔 疯情书库 .ruleContent.content
java
  .ajax(
    baseUrl
      .replace("read-", "_getcontent.php?id=")
      .replace(".html", "&v=" + result.match(/&v=(.*?)"/)[1])
  )
  .replaceAll("<style.*style>", "")
  .replaceAll("<([^<]*?)class(.*?)</(.*?)>", "");
