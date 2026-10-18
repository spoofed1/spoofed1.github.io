const button=document.querySelector('#connect'),cancel=document.querySelector('#cancel'),status=document.querySelector('#status');
const stages={trust:'Unlock your iPhone and approve Trust if asked.',checking:'Checking that Spoofed is ready to receive…',pairing:'Creating your trusted pairing…',sending:'Sending the pairing directly to Spoofed…',delivered:'Sent to Spoofed. Check the app for “Pairing received”, then connect LocalDevVPN.'};
let busy=false,cancelled=false,transport,engine;
function message(text){status.textContent=text;}
addEventListener('spoofed-stage',event=>{if(busy&&stages[event.detail])message(stages[event.detail]);});
const supported=globalThis.isSecureContext&&'usb' in navigator;
if(supported){message('Ready when you are.');button.disabled=false;}else{message('Open this page in desktop Chrome or Edge to connect over USB.');}
button.addEventListener('click',async()=>{
 if(busy)return;busy=true;cancelled=false;button.disabled=true;cancel.hidden=false;
 try{
  // The chooser must run directly inside the click, before fetching the engine.
  const selected=await navigator.usb.requestDevice({filters:[{vendorId:0x05ac}]});
  if(cancelled)throw Error('Setup cancelled.');
  message('Loading the local pairing engine…');
  engine=await import('./pkg/spoofed_web.js');await engine.default();
  // Import the exact module used by wasm-bindgen: it owns the shared USB session.
  transport=await import('./transport-entry.js');
  if(cancelled)throw Error('Setup cancelled.');
  await transport.openUSB(selected);
  if(cancelled)throw Error('Setup cancelled.');
  message(stages.trust);
  await engine.pair_and_deliver();
 }catch(error){
  if(error?.name==='NotFoundError')message('No iPhone selected. Click Connect to try again.');
  else message('Setup stopped. '+(error?.message||String(error))+ ' Keep Receive from Computer open in Spoofed, then retry.');
 }finally{if(transport)await transport.closeUSB();busy=false;button.disabled=!supported;cancel.hidden=true;}
});
cancel.addEventListener('click',async()=>{cancelled=true;message('Disconnecting…');if(transport)await transport.closeUSB();});
addEventListener('pagehide',()=>{if(transport)void transport.closeUSB();});
