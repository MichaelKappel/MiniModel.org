"use strict";
const assert=require("node:assert/strict"),fs=require("node:fs"),path=require("node:path"),
 crypto=require("node:crypto"),{spawnSync}=require("node:child_process"),test=require("node:test");
const root=path.resolve(__dirname,"..");
const api=require("../../TinyRustLM.com/tinyrustlm/app/minimodel-catalog-v1.js");
const mini=require("../catalog.js");
const demo=require("../../TinyRustLM.com/tinyrustlm/tools/synthetic-p2p-interview-demo.js");
const {createOwnedTemporaryDirectory,removeOwnedTemporaryDirectory}=
 require("../../TinyRustLM.com/tinyrustlm/tools/canonical-operation-scratch.js");
const canon=x=>JSON.stringify(x)+"\n",sha=b=>crypto.createHash("sha256").update(b).digest("hex");
const seed=Buffer.alloc(32,1); // Public deterministic fixture only.
const key=crypto.createPrivateKey({key:Buffer.concat([Buffer.from("302e020100300506032b657004220420","hex"),seed]),format:"der",type:"pkcs8"});
const publicKey=crypto.createPublicKey(key);
const utc=t=>new Date(t*1000).toISOString().replace(".000Z","Z");
function fixture(base,name,age=72000) {
 const now=Math.floor(Date.now()/1000),site=path.join(base,name),priv=path.join(site,".minimodel-metadata-renewal.v1/private");
 fs.mkdirSync(priv,{recursive:true});
 const source=demo.createSyntheticFixture("renewal-test-never-live",demo.PRESERVED_PRIOR_LENGTHS,0);
 const docs=demo.createDemoDocuments([source],48123);
 const c=JSON.parse(canon(docs.catalog)),f=JSON.parse(canon(docs.peerFeed));
 c.project_server_serves_model_bytes=f.project_server_serves_model_bytes=true;
 c.peer_verification_keys[0].public_key_base64url=publicKey.export({format:"jwk"}).x;
 c.peer_verification_keys[0].not_before_utc=utc(now-30*86400);
 c.peer_verification_keys[0].not_after_utc=utc(now+365*86400);
 c.generated_at_utc=f.generated_at_utc=utc(now-age);
 c.valid_until_utc=f.valid_until_utc=utc(now-age+86400);
 f.announcements=[f.announcements[0]];
 c.entries[0].peer_announcement_ids=[f.announcements[0].announcement_id];
 Object.assign(f.announcements[0],{peer_base_url:"https://minimodel.org/",outside_proof_status:"not-required-operator-seed",outside_proof_receipt_sha256:null,
 issued_at_utc:c.generated_at_utc,expires_utc:c.valid_until_utc});
 f.announcements[0].signature=crypto.sign(null,api.peerAnnouncementSigningBytes(f.announcements[0]),key).toString("base64url");
 const refs=Array.from({length:9},(_,i)=>{const bytes=Buffer.from("synthetic-proof-"+i+"\n"),p="evidence-"+i+".txt";
 fs.writeFileSync(path.join(site,p),bytes);return{path:p,bytes:bytes.length,sha256:sha(bytes)};});
 const config={schema:"minimodel.metadata_renewal_config.v1",initial_catalog:canon(c),initial_peer_feed:canon(f),
 release_owner:{site:"minimodel.org",release_id:"synthetic-test-never-live",source_commit:"a".repeat(40),publisher_commit:"b".repeat(40),manifest_sha256:"c".repeat(64),authority_sha256:"d".repeat(64)},
 source_references:refs,ttl_seconds:86400,refresh_before_seconds:21600};
 const configBytes=canon(config);
 const pair={schema:"minimodel.metadata_pair.v1",config_sha256:sha(configBytes),catalog:canon(c),peer_feed:canon(f)};
 fs.writeFileSync(path.join(priv,"config.json"),configBytes);
 fs.writeFileSync(path.join(priv,"pair.json"),canon(pair));
 fs.writeFileSync(path.join(priv,".htaccess"),"Require all denied\n");
 fs.writeFileSync(path.join(priv,"signing-key.seed"),seed);
 fs.writeFileSync(path.join(priv,"renewal.lock"),"");
 return{site,priv,pair,c,f,config};
}
test("renewal source manifest binds its exact four deployed files",()=>{
 const dir=path.join(root,"deploy/host-renewal"),m=JSON.parse(fs.readFileSync(path.join(dir,"source-manifest.v1.json")));
 assert.equal(m.published,false);assert.equal(m.remoteNamespace,".minimodel-metadata-renewal.v1");
 assert.deepEqual(m.members.map(x=>x.path),[".htaccess","private/.htaccess","renewal-core.php","metadata.php"]);
 for(const x of m.members){const b=fs.readFileSync(path.join(dir,x.path));assert.equal(b.length,x.byteLength);assert.equal(sha(b),x.sha256);}
});
test("actual PHP renews a signed pair and rejects unsafe state; outputs pass unchanged JS readers",{
 skip:process.env.MINIMODEL_HOST_RENEWAL_PHP_DOCKER!=="1"?"Explicit cached PHP runtime not selected":false
},()=>{
 const image=spawnSync("docker",["image","inspect","php:8.3-cli","--format","{{.Id}}"],{encoding:"utf8",timeout:15000,windowsHide:true});
 assert.equal(image.status,0,image.stderr);assert.match(image.stdout.trim(),/^sha256:[a-f0-9]{64}$/);
 const owned=createOwnedTemporaryDirectory({temporaryParent:path.resolve(root,"../TinyRustLM.com/artifacts/tests/minimodel-host-renewal")});
 try{
 const cases=[];
 function add(name,age,mutate,expected=200,renew=false,request={}) {
  const x=fixture(owned.rootPath,name,age);if(mutate)mutate(x);
  cases.push({name,expected,renew,initialRevision:x.c.revision,...request});
 }
 add("fresh",0,null,200,false);
 add("renew",72000,null,200,true);
 add("expired-renews",90000,null,200,true);
 const foreign=x=>{const p=path.join(x.site,".minimodel-deployment-lease.v1");fs.mkdirSync(p);fs.writeFileSync(path.join(p,"foreign"),"retain");};
 add("busy-valid",72000,foreign);
 add("busy-expired",90000,foreign,503);
 add("journal-retained",72000,x=>fs.writeFileSync(path.join(x.priv,"pending.json"),"retain"));
 add("source-corrupt",90000,x=>fs.writeFileSync(path.join(x.site,"evidence-0.txt"),"bad"),503);
 add("wrong-key",90000,x=>fs.writeFileSync(path.join(x.priv,"signing-key.seed"),Buffer.alloc(32,2)),503);
 const pairChange=(x,fn)=>{const p=JSON.parse(fs.readFileSync(path.join(x.priv,"pair.json")));fn(p);fs.writeFileSync(path.join(x.priv,"pair.json"),canon(p));};
 add("bad-signature",0,x=>pairChange(x,p=>{const f=JSON.parse(p.peer_feed);f.announcements[0].signature=Buffer.alloc(64).toString("base64url");p.peer_feed=canon(f);}),503);
 add("immutable-change",0,x=>pairChange(x,p=>{const c=JSON.parse(p.catalog);c.entries[0].display_name="replacement";p.catalog=canon(c);}),503);
 add("wrong-config-binding",0,x=>pairChange(x,p=>p.config_sha256="0".repeat(64)),503);
 add("private-denial-missing",0,x=>fs.writeFileSync(path.join(x.priv,".htaccess"),"Require all granted\n"),503);
 add("noncanonical-pair",0,x=>fs.appendFileSync(path.join(x.priv,"pair.json")," "),503);
 add("head",0,null,200,false,{method:"HEAD"});
 add("post",0,null,405,false,{method:"POST"});
 add("query",0,null,400,false,{query:"path=other",uri:"/catalog/compositions.v1.json?path=other"});
 add("empty-query",0,null,400,false,{uri:"/catalog/compositions.v1.json?"});
 add("direct-handler",0,null,404,false,{uri:"/.minimodel-metadata-renewal.v1/metadata.php"});
 add("peer-route",72000,null,200,true,{uri:"/catalog/peer-announcements.v1.json"});
 fs.writeFileSync(path.join(owned.rootPath,"cases.json"),canon(cases));
 const php='<?php\nrequire "/source/deploy/host-renewal/renewal-core.php";\n'+
 '$out=[]; foreach(json_decode(file_get_contents("/renewal-tests/cases.json")) as $c){'+
 '$input="/renewal-tests/".$c->name;$base="/renewal-runtime/".$c->name;mkdir($base,0700);'+
 'foreach(new RecursiveIteratorIterator(new RecursiveDirectoryIterator($input,FilesystemIterator::SKIP_DOTS),RecursiveIteratorIterator::SELF_FIRST) as $item){'+
 '$dest=$base.substr($item->getPathname(),strlen($input));if($item->isLink()){throw new RuntimeException("fixture link");}'+
 'if($item->isDir()){mkdir($dest,0700);}else{if(!copy($item->getPathname(),$dest)){throw new RuntimeException("fixture copy");}chmod($dest,0600);}}'+
 '$root=$base."/.minimodel-metadata-renewal.v1";'+
 '[$status,$body]=mmRenewMetadata($root,$c->method??"GET",$c->query??"",$c->uri??"/catalog/compositions.v1.json");'+
 '$out[]=["name"=>$c->name,"status"=>$status,"body"=>$body,"pair"=>file_get_contents($root."/private/pair.json"),'+
 '"leaseExists"=>is_dir(dirname($root)."/.minimodel-deployment-lease.v1"),"journalExists"=>file_exists($root."/private/pending.json")];}'+
 'echo json_encode($out,JSON_THROW_ON_ERROR),"\\n";';
 const r=spawnSync("docker",["run","--rm","-i","--pull=never","--network","none","--read-only","--cap-drop","ALL","--security-opt","no-new-privileges",
 "--tmpfs","/renewal-runtime:rw,noexec,nosuid,size=16777216",
 "--mount","type=bind,src="+root+",dst=/source,readonly","--mount","type=bind,src="+owned.rootPath+",dst=/renewal-tests",image.stdout.trim(),"php"],
 {input:php,encoding:"utf8",timeout:90000,maxBuffer:4*1024*1024,windowsHide:true});
 assert.equal(r.status,0,r.stdout+r.stderr);assert.equal(r.stderr,"");
 const output=JSON.parse(r.stdout),summaries=[];
 for(const actual of output){const planned=cases.find(x=>x.name===actual.name);
 assert.equal(actual.status,planned.expected,actual.name);
 if(actual.status===200){
 const pair=JSON.parse(actual.pair),c=JSON.parse(pair.catalog),f=JSON.parse(pair.peer_feed);
 assert.equal(c.revision,planned.initialRevision+(planned.renew?1:0),actual.name+" revision");
 const checked=mini.validateCatalog(c,{now:new Date()});mini.validatePeerFeed(f,checked,{now:new Date()});
 const a=f.announcements[0];assert(crypto.verify(null,api.peerAnnouncementSigningBytes(a),publicKey,Buffer.from(a.signature,"base64url")));
 const field=planned.uri==="/catalog/peer-announcements.v1.json"?"peer_feed":"catalog";
 assert.equal(actual.body,pair[field]);if(planned.renew){assert(!actual.leaseExists);assert(!actual.journalExists);}
 }
 if(actual.name.startsWith("busy"))assert(actual.leaseExists);
 if(actual.name==="journal-retained")assert(actual.journalExists);
 summaries.push({name:actual.name,status:actual.status,renewed:planned.renew});
 }
 console.log(JSON.stringify({schema:"minimodel.host_renewal_local_test.v1",image:image.stdout.trim(),cases:summaries,productionKey:false,published:false}));
 }finally{removeOwnedTemporaryDirectory(owned);}
});
