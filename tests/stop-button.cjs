const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const script = fs.readFileSync('src/index.html', 'utf8').match(/<script>([\s\S]*?)<\/script>/)[1];
const elements = Object.fromEntries(['status','start','stop','pick','x','y','w','h','i','t','d'].map(id => [id,{value:10,disabled:false,textContent:''}]));
let running = false, stops = 0;
const browserStop = function() {};
const context = {
 document: {getElementById: id => elements[id]},
 window: {stop:browserStop,__TAURI__:{core:{invoke:async name=>{
  if(name==='start_monitor'){if(running)throw '监测已在运行';running=true;}
  if(name==='stop_monitor'){running=false;stops++;}
 }},event:{listen:()=>{}}}}
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
 console.log('PASS: duplicate start -> stop -> repeated stop -> restart; window.stop untouched');
})().catch(e=>{console.error(e);process.exitCode=1});
