const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const script = fs.readFileSync('src/index.html', 'utf8').match(/<script>([\s\S]*?)<\/script>/)[1];
const elements = Object.fromEntries(['status','start','stop','pick','testNotification','x','y','w','h','i','t','d'].map(id => [id,{value:10,disabled:false,textContent:'',after:()=>{}}]));
const notificationStatus = {textContent:''};
const listeners = {};
let notificationError = null, testCalls = 0;
let running = false, stops = 0;
const browserStop = function() {};
const context = {
 document: {getElementById: id => elements[id], createElement:()=>notificationStatus},
 window: {stop:browserStop,__TAURI__:{core:{invoke:async name=>{
  if(name==='start_monitor'){if(running)throw '监测已在运行';running=true;}
  if(name==='stop_monitor'){running=false;stops++;}
  if(name==='test_notification'){testCalls++;if(notificationError)throw notificationError;}
 }},event:{listen:(name,callback)=>{listeners[name]=callback}}}}
};
vm.runInNewContext(script,context);
(async()=>{
 assert.equal(browserStop.onclick,undefined);
 assert.equal(typeof elements.stop.onclick,'function');
 await elements.start.onclick();
 await elements.start.onclick();
 assert.equal(elements.status.textContent,'监测已在运行');
 await elements.stop.onclick();
 assert.equal(running,false);
 assert.equal(elements.status.textContent,'已停止');
 await elements.stop.onclick();
 await elements.start.onclick();
 assert.equal(running,true);
 assert.equal(stops,2);
 await elements.stop.onclick();
 for(const value of [255,765,-1,NaN]) {
  elements.t.value=value;
  await elements.start.onclick();
  assert.equal(running,false);
  assert.match(elements.status.textContent,/0–254/);
 }
 elements.t.value=75;
 await elements.start.onclick();
 assert.equal(running,true);
 await elements.testNotification.onclick();
 assert.match(notificationStatus.textContent,/已提交给 Windows/);
 assert.equal(elements.testNotification.disabled,false);
 notificationError='通知已被 Windows 组策略禁用';
 await elements.testNotification.onclick();
 assert.match(notificationStatus.textContent,/测试通知失败.*组策略/);
 assert.equal(elements.testNotification.disabled,false);
 listeners['notification-error']({payload:'异步发送失败'});
 listeners['region-changed']({});
 assert.equal(notificationStatus.textContent,'通知失败：异步发送失败');
 assert.equal(elements.status.textContent,'检测到持续变化');
 notificationError=null;
 await elements.testNotification.onclick();
 assert.match(notificationStatus.textContent,/已提交给 Windows/);
 assert.equal(testCalls,3);
 console.log('PASS: start/stop/restart, invalid thresholds, notification submission, synchronous and asynchronous failures');
})().catch(e=>{console.error(e);process.exitCode=1});
