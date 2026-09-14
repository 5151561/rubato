// from: 晋江① .ruleBookInfo.intro
if(/完结/.test(book.kind))book.canUpdate=false;
a=JSON.parse(java.ajax('http://app.jjwxc.org/androidapi/getnovelOtherInfo?novelId='+java.get('id')+'&type=novelbasicinfo&versionCode=148'));
b=a.novelLeave;
c=JSON.parse(result);
z=s=>s.length>3?s:'';
['',b.leaveDateBack,b.leaveContent,b.leaveDate,' '+c.novelIntroShort,'​','标签：'+c.novelTags,z(c.protagonist),z(c.costar),z(c.other),'风格：'+c.novelStyle+'　　视角：'+c.mainview,'收藏：'+a.novelbefavoritedcount+'　评论：'+c.comment_count+'　评分：'+c.novelReviewScore,'┄┄',c.novelIntro].join('\n').replace('立意:','┄┄\n立意：').replace('评分：\n','\n').replace(' \n','')
