// from: 69书吧.com .ruleBookInfo.init
if(result.match(/^<!DOCTYPE html><html lang="en-US"><head><title>Just a moment...</)){java.longToast('请根据网页提示点击勾选「确认您是真人」来通过人机验证，如果无限循环请查看源注释说明。');result=java.startBrowserAwait(baseUrl,'人机验证').body();};result;
