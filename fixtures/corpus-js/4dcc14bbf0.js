// from: 番茄小说2 .searchUrl
function getUrl(key) {
  let isNumber = /^\d+$/.test(key);
  if (isNumber) {
    return `https://api5-normal-sinfonlineb.fqnovel.com/reading/bookapi/multi-detail/v/?aid=1967&iid=1&version_code=999&book_id={{key}}`;
  } else {
    return `https://api5-normal-lf.fqnovel.com/reading/bookapi/search/page/v/?passback={{(page-1)*50}}&query={{key}}$&iid=1&tab_type=3&channel=0&aid=1967&app_name=novelapp&version_code=99999&device_platform=linux&device_type=WSA3&language=zh&os_version=8.1.0`;
  }
}
getUrl(key)
