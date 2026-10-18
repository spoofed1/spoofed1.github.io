// Direct-usbmux framing adapted from RorkDeviceWeb (Apache-2.0).
// Modified for JavaScript/WebUSB; see THIRD_PARTY_NOTICES.md.
let mux;
const empty = new Uint8Array();
function concat(a,b){const r=new Uint8Array(a.length+b.length);r.set(a);r.set(b,a.length);return r;}
function gate(ms=30000){let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});const timer=setTimeout(()=>reject(new Error('The phone did not respond. Unlock it and try again.')),ms);promise.then(()=>clearTimeout(timer),()=>clearTimeout(timer));return {promise,resolve,reject};}
export function reportStage(stage){globalThis.dispatchEvent(new CustomEvent('spoofed-stage',{detail:stage}));}
export async function openUSB(device){mux=new Mux(device);try{await mux.open();}catch(e){await mux.close();throw e;}}
export async function closeUSB(){const old=mux;mux=null;if(old)await old.close();}
export async function connectPort(port){if(!mux)throw Error('Connect the phone first.');return mux.connect(port);}
export async function readPort(id,size){return mux.read(id,size);}
export async function writePort(id,data){return mux.write(id,new Uint8Array(data));}
export function closePort(id){if(mux)mux.drop(id);}
export class Mux {
 constructor(device){this.device=device;this.streams=new Map();this.next=49152;this.tx=0;this.rx=65535;this.version=0;this.buffer=empty;this.out=Promise.resolve();this.closed=false;}
 async open(){
  await this.device.open();let found;
  for(const c of this.device.configurations)for(const i of c.interfaces)for(const a of i.alternates){
   if(a.interfaceClass!==255||a.interfaceSubclass!==254||a.interfaceProtocol!==2)continue;
   const input=a.endpoints.find(e=>e.direction==='in'&&e.type==='bulk');const output=a.endpoints.find(e=>e.direction==='out'&&e.type==='bulk');
   if(input&&output){found={c,i,a,input,output};break;}
  }
  if(!found)throw Error('This phone does not expose a compatible USB interface.');
  const {c,i,a,input,output}=found;this.interface=i.interfaceNumber;this.input=input.endpointNumber;this.output=output.endpointNumber;this.packetSize=output.packetSize;
  if(this.device.configuration?.configurationValue!==c.configurationValue)await this.device.selectConfiguration(c.configurationValue);
  try{await this.device.claimInterface(this.interface);}catch{throw Error('USB is busy or unavailable. Close Xcode, Finder device windows, and other phone tools, reconnect the cable, then retry. This computer may need a compatible USB driver.');}
  await this.device.selectAlternateInterface(this.interface,a.alternateSetting);
  this.ready=gate(10000);this.loop().catch(e=>this.fail(e));
  const version=new Uint8Array(12);new DataView(version.buffer).setUint32(0,2);await this.send(0,version);await this.ready.promise;
  if(this.version===2){this.tx=0;this.rx=65535;await this.send(2,new Uint8Array([7]));}
 }
 send(protocol,payload){const task=this.out.then(async()=>{
  if(this.closed)throw Error('USB disconnected.');const h=this.version===2?16:8;const b=new Uint8Array(h+payload.length);const v=new DataView(b.buffer);
  v.setUint32(0,protocol);v.setUint32(4,b.length);if(h===16){v.setUint32(8,0xfeedface);v.setUint16(12,this.tx++ & 65535);v.setUint16(14,this.rx);}b.set(payload,h);
  const result=await this.device.transferOut(this.output,b);if(result.status!=='ok'||result.bytesWritten!==b.length)throw Error('USB write failed.');
 });this.out=task.catch(()=>{});return task;}
 async loop(){while(!this.closed){const r=await this.device.transferIn(this.input,16384);if(r.status!=='ok'||!r.data)throw Error('USB read failed.');this.buffer=concat(this.buffer,new Uint8Array(r.data.buffer,r.data.byteOffset,r.data.byteLength));
  while(this.buffer.length>=(this.version===2?16:8)){
   const h=this.version===2?16:8;const v=new DataView(this.buffer.buffer,this.buffer.byteOffset);const n=v.getUint32(4);const p=v.getUint32(0);if(n<h||n>262144)throw Error('Invalid USB frame.');if(this.buffer.length<n)break;
   const bytes=this.buffer.slice(h,n);if(h===16)this.rx=v.getUint16(14);this.buffer=this.buffer.slice(n);
   if(p===0&&!this.version){if(bytes.length<12)throw Error('Invalid USB handshake.');this.version=new DataView(bytes.buffer).getUint32(0);if(![1,2].includes(this.version))throw Error('Unsupported USB protocol.');this.ready.resolve();}
   else if(p===6)await this.receive(bytes);
  }
 }}
 tcp(s,flags,payload=empty,seq=s.seq){const b=new Uint8Array(20+payload.length),v=new DataView(b.buffer);v.setUint16(0,s.local);v.setUint16(2,s.remote);v.setUint32(4,seq);v.setUint32(8,s.ack);b[12]=0x50;b[13]=flags;v.setUint16(14,512);b.set(payload,20);return this.send(6,b);}
 async connect(port){if(this.closed)throw Error('USB disconnected.');if(this.next>65000)throw Error('Too many service connections. Reconnect.');const s={local:this.next++,remote:port,seq:0,ack:0,peerAck:0,window:0,buffer:empty,connected:false,gate:gate(10000),reader:null,writer:null,error:null};this.streams.set(s.local,s);await this.tcp(s,2);await s.gate.promise;return s.local;}
 async receive(bytes){if(bytes.length<20)throw Error('Invalid service frame.');const v=new DataView(bytes.buffer,bytes.byteOffset);const source=v.getUint16(0),local=v.getUint16(2),seq=v.getUint32(4),ack=v.getUint32(8),flags=bytes[13],offset=(bytes[12]>>4)*4;
  if(offset<20||offset>bytes.length)throw Error('Invalid service header.');const s=this.streams.get(local);if(!s||s.remote!==source)return;
  if(flags&5){this.failStream(s,Error('The phone closed a service. Keep it unlocked and retry.'));return;}
  if(!s.connected){if(flags!==18||ack!==1){this.failStream(s,Error('USB service refused connection.'));return;}s.connected=true;s.seq=1;s.ack=(seq+1)>>>0;s.peerAck=ack;s.window=v.getUint16(14)*256;await this.tcp(s,16);s.gate.resolve();return;}
  const ahead=(ack-s.seq)|0;if(ahead>0)throw Error('Invalid service acknowledgement.');if(((ack-s.peerAck)|0)>=0){s.peerAck=ack;s.window=v.getUint16(14)*256;s.writer?.resolve();s.writer=null;}
  const data=bytes.slice(offset);if(!data.length)return;if(seq!==s.ack){if(((s.ack-seq)|0)>0){await this.tcp(s,16);return;}throw Error('USB data arrived out of order.');}
  s.ack=(s.ack+data.length)>>>0;s.buffer=concat(s.buffer,data);if(s.buffer.length>8*1024*1024)throw Error('USB buffer limit exceeded.');await this.tcp(s,16);s.reader?.resolve();s.reader=null;
 }
 state(id){const s=this.streams.get(id);if(!s)throw Error('USB service is closed.');if(s.error)throw s.error;return s;}
 async read(id,size){const s=this.state(id);while(!s.buffer.length){s.reader=gate(120000);await s.reader.promise;this.state(id);}const data=s.buffer.slice(0,size);s.buffer=s.buffer.slice(data.length);return data;}
 async write(id,data){const s=this.state(id);let off=0;while(off<data.length){this.state(id);const available=s.window-((s.seq-s.peerAck)>>>0);if(available<=0){s.writer=gate();await s.writer.promise;continue;}let n=Math.min(available,data.length-off,16384);const h=this.version===2?36:28;if((n+h)%this.packetSize===0&&n>1)n--;const seq=s.seq;s.seq=(s.seq+n)>>>0;await this.tcp(s,16,data.slice(off,off+n),seq);off+=n;}}
 failStream(s,error){s.error=error;s.gate.reject(error);s.reader?.reject(error);s.writer?.reject(error);}
 fail(error){this.ready?.reject(error);for(const s of this.streams.values())this.failStream(s,error);this.close().catch(()=>{});}
 drop(id){const s=this.streams.get(id);if(!s)return;this.tcp(s,4).catch(()=>{});this.failStream(s,Error('USB service closed.'));this.streams.delete(id);}
 async close(){if(this.closed)return;this.closed=true;this.ready?.reject(Error('USB disconnected.'));for(const s of this.streams.values())this.failStream(s,Error('USB disconnected.'));this.streams.clear();try{await this.device.releaseInterface(this.interface);}catch{}try{await this.device.reset();}catch{}try{await this.device.close();}catch{}}
}
