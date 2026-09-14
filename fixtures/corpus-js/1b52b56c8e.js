// from: 🌾色彩夏书 .ruleToc.chapterList
//二级目录，Json
var sid = java.get('sid');
var bid = java.get('bid');
var jdoc = JSON.parse(result);
var chUrl = 'https://contentxs.pigqq.com/BookFiles/Html/'+sid+'/'+bid+'/';
var volUrl = 'https://translate.google.cn/#view=home&op=translate&sl=zh-CN&tl=en&text=';
var list = [];


//章节名显示卷名
var opts = {
  showVolName: false
};

sm=java.get('sm');

function format(title){
  return title.trim().replace(/^(\d+)(?![\s\d卷部章节回.])/, '$1 ').replace(/\s+/g, "\x20\x20");
}

jdoc.forEach((vol,idx) => {
  var chs = vol.list.map(ch => ({
  	text: !vol.name?format(ch.name):((opts.showVolName ? format(vol.name) + '☪' :(ch.IsVip === '1' ? '❍·' : '').padStart(3, ""))+format(ch.name).replace(/^(正文|VIP章节|最新章节)?(\s+|_)|\s\S*[求更谢乐发推票盟补加字Kk\/]\S*\s/g,'')),
    href: chUrl+ch.id+'.html',
    name: format(ch.name),
  }));
  
  if(vol.name!=""){
  var volInfo = sm+'\\'+ format(vol.name);
  list.push({
    text: '————◆ [' + format(vol.name) + '] ◆————',
    href: volUrl + encodeURIComponent(volInfo),

    VolName:true
    });
    }
  list = list.concat(chs);
});
result = list;
