// from: 🔖 百度知道 .exploreUrl
//一行个数
var nums = 4 ;
//分隔符
var separator = '::'

//标题::链接::一行个数::分类

all=[
"----------     🏷标签     ----------::::1",
"进击的巨人::进击的巨人::::a"
];

//@天天gg@酷安 下面不要动
function parse(data){
	let args=data.split(separator);
	let title = args[0],url=args[1],num=nums;
	
	//链接生成
	 let u=/^\d+/.test(url)?"https://zhidao.baidu.com/msearch/ajax/getsearchlist?word="+url+"&pn=10":url;
 try {num = args[2] }catch(e){}
 try {tag = args[3];
 u=tag=="a"?'https://zhidao.baidu.com/msearch/ajax/getsearchlist?word='+url+'&pn=10':u;

 }catch(e){}
	return [title, u, num]
	}

function FlexBox(title, url, num){
		 let obj={};
		 obj.style={},obj.title=title,obj.url=url?url:'',obj.style['layout_flexGrow']=1;
	//数值设定
	let data={1:1,2:0.4,3:0.25,4:0.2,5:0.15,7:0.1,10:0.05};
obj.style['layout_flexBasisPercent']=data[num]
		return obj
		}
result=JSON.stringify(all.map(data=>{
	let args=parse(data);
	return FlexBox.apply(null, args)
	}))
