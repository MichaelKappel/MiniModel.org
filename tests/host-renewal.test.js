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
 // Plain synthetic evidence bytes; no usable model or production qualification.
 const refs=[],entry=c.entries[0],prefix="tinyrustlm/direct-composition/v1/compositions/"+entry.composition.composition_id+"/members/";
 const save=(route,bytes)=>{const index=refs.length;fs.writeFileSync(path.join(site,"evidence-"+index+".txt"),bytes);
 const ref={path:route,bytes:bytes.length,sha256:sha(bytes)};refs.push(ref);return ref;};
 entry.composition.members.forEach((member,i)=>{const bytes=Buffer.alloc(i===5?896:i===4?512:32,65+i);
 const ref=save(prefix+member.name,bytes);member.bytes=ref.bytes;member.sha256="sha256:"+ref.sha256;});
 let ref=save(entry.browser_admission.route.slice(1),Buffer.from(source.admissionText));
 entry.browser_admission.bytes=ref.bytes;entry.browser_admission.sha256="sha256:"+ref.sha256;
 ref=save(entry.qualification.receipt_route.slice(1),Buffer.from(source.qualificationReceiptText));
 entry.qualification.receipt_bytes=ref.bytes;entry.qualification.receipt_sha256="sha256:"+ref.sha256;
 save(entry.license.route.slice(1),Buffer.from("Synthetic test license only\n"));
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
 const cases=[];let policyBase;
 function add(name,age,mutate,expected=200,renew=false,request={}) {
  const x=fixture(owned.rootPath,name,age);if(name==="fresh")policyBase={entry:x.c.entries[0],refs:x.config.source_references};if(mutate)mutate(x);
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
 // Change config and its digest together: closure validation must reject bad publisher inputs.
 const configChange=(x,fn)=>{const cfg=JSON.parse(fs.readFileSync(path.join(x.priv,"config.json")));fn(cfg);
 const bytes=canon(cfg);fs.writeFileSync(path.join(x.priv,"config.json"),bytes);
 pairChange(x,p=>p.config_sha256=sha(bytes));};
 add("wrong-seed-reference",0,x=>configChange(x,cfg=>cfg.source_references[0].path=cfg.source_references[0].path.replace("sha256:","sha256:0")),503);
 add("wrong-source-hash",0,x=>configChange(x,cfg=>cfg.source_references[0].sha256="0".repeat(64)),503);
 add("missing-source",0,x=>configChange(x,cfg=>cfg.source_references.pop()),503);
 add("extra-source",0,x=>configChange(x,cfg=>cfg.source_references.push({path:"extra",bytes:1,sha256:"0".repeat(64)})),503);
 add("wrong-admission-reference",0,x=>configChange(x,cfg=>cfg.source_references[6].path="other.json"),503);
 add("wrong-receipt-reference",0,x=>configChange(x,cfg=>cfg.source_references[7].bytes++),503);
 add("source-symlink",90000,null,503,false,{linuxMutation:"source-symlink"});
 add("source-hardlink",90000,null,503,false,{linuxMutation:"source-hardlink"});
 add("parent-symlink",90000,null,503,false,{linuxMutation:"parent-symlink"});
 add("held-lock-valid",72000,null,200,false,{linuxMutation:"held-lock"});
 add("interrupted-next-retained",72000,x=>fs.writeFileSync(path.join(x.priv,"pair.next"),"retain"));
 add("concurrent-expired",90000,null,200,true,{linuxMutation:"concurrent"});
 add("head",0,null,200,false,{method:"HEAD"});
 add("post",0,null,405,false,{method:"POST"});
 add("query",0,null,400,false,{query:"path=other",uri:"/catalog/compositions.v1.json?path=other"});
 add("empty-query",0,null,400,false,{uri:"/catalog/compositions.v1.json?"});
 add("direct-handler",0,null,404,false,{uri:"/.minimodel-metadata-renewal.v1/metadata.php"});
 add("peer-route",72000,null,200,true,{uri:"/catalog/peer-announcements.v1.json"});
 // Pure metadata limits: no large payload is allocated or claimed to have executed.
 const policies=[];
 const policy=(name,mutate,ok)=>{const x=JSON.parse(canon(policyBase));if(mutate)mutate(x);policies.push({name,...x,ok});};
 policy("exact-closure",null,true);
 policy("two-gib-model",x=>{x.entry.composition.members[0].bytes=x.refs[0].bytes=2147483648;},true);
 const caps=[2147483648,67108864,1048576,65800,512,896];
 caps.forEach((cap,i)=>policy("over-limit-"+i,x=>{x.entry.composition.members[i].bytes=x.refs[i].bytes=cap+1;},false));
 policy("wrong-graph-size",x=>{x.entry.composition.members[5].bytes=x.refs[5].bytes=895;},false);
 policy("zero-model",x=>{x.entry.composition.members[0].bytes=x.refs[0].bytes=0;},false);
 policy("noninteger-size",x=>{x.entry.composition.members[0].bytes=x.refs[0].bytes=1.5;},false);
 policy("member-order",x=>{x.entry.composition.members.reverse();},false);
 for(const route of ["/other:seed/license.txt","/proof/../key","/proof//license","//host/license","/.minimodel-metadata-renewal.v1/private/key","/tinyrustlm/direct-composition/license.txt"]){
 policy("unsafe-route:"+route,x=>{x.entry.license.route=route;x.refs[8].path=route.slice(1);},false);}
 policy("duplicate-reference",x=>{x.entry.license.route=x.entry.browser_admission.route;x.refs[8].path=x.refs[6].path;},false);
 policy("wrong-digest",x=>{x.refs[0].sha256="0".repeat(64);},false);
 fs.writeFileSync(path.join(owned.rootPath,"policies.json"),canon(policies));
 fs.writeFileSync(path.join(owned.rootPath,"cases.json"),canon(cases));
 const php='<?php\nrequire "/source/deploy/host-renewal/renewal-core.php";\n'+
 '$out=[]; foreach(json_decode(file_get_contents("/renewal-tests/cases.json")) as $c){'+
 '$input="/renewal-tests/".$c->name;$base="/renewal-runtime/".$c->name;mkdir($base,0700);'+
 'foreach(new RecursiveIteratorIterator(new RecursiveDirectoryIterator($input,FilesystemIterator::SKIP_DOTS),RecursiveIteratorIterator::SELF_FIRST) as $item){'+
 '$dest=$base.substr($item->getPathname(),strlen($input));if($item->isLink()){throw new RuntimeException("fixture link");}'+
 'if($item->isDir()){mkdir($dest,0700);}else{if(!copy($item->getPathname(),$dest)){throw new RuntimeException("fixture copy");}chmod($dest,0600);}}'+
 '$root=$base."/.minimodel-metadata-renewal.v1";'+
 '$cfg=json_decode(file_get_contents($root."/private/config.json"));'+
 // Canonical colon-bearing remote paths are constructed only inside PHP-owned Linux tmpfs.
 'foreach($cfg->source_references as $i=>$ref){$src=$base."/evidence-".$i.".txt";'+
 'if(!is_file($src)){continue;}$dest=$base."/".$ref->path;if(!is_dir(dirname($dest))){mkdir(dirname($dest),0700,true);}rename($src,$dest);}'+
 '$mutation=$c->linuxMutation??"";$held=null;'+
 'if($mutation==="source-symlink"||$mutation==="source-hardlink"){$p=$base."/".$cfg->source_references[0]->path;rename($p,$base."/target");'+
 'if($mutation==="source-symlink"){symlink($base."/target",$p);}else{link($base."/target",$p);}}'+
 'if($mutation==="parent-symlink"){$p=dirname($base."/".$cfg->source_references[0]->path);rename($p,$base."/target-dir");symlink($base."/target-dir",$p);}'+
 'if($mutation==="held-lock"||$mutation==="concurrent"){$held=fopen($root."/private/renewal.lock","rb");flock($held,LOCK_EX);}'+
 '$concurrent=[];if($mutation==="concurrent"){$children=[];'+
 'for($i=0;$i<2;$i++){$pipes=[];$code="require \\"/source/deploy/host-renewal/renewal-core.php\\";echo json_encode(mmRenewMetadata(\\$argv[1],\\"GET\\",\\"\\",\\"/catalog/compositions.v1.json\\"));";'+
 '$proc=proc_open([PHP_BINARY,"-r",$code,$root],[0=>["pipe","r"],1=>["pipe","w"],2=>["pipe","w"]],$pipes);fclose($pipes[0]);$children[]=[$proc,$pipes];}'+
 'usleep(250000);flock($held,LOCK_UN);fclose($held);$held=null;foreach($children as [$proc,$pipes]){$text=stream_get_contents($pipes[1]);$err=stream_get_contents($pipes[2]);fclose($pipes[1]);fclose($pipes[2]);'+
 'if(proc_close($proc)!==0||$err!==""){throw new RuntimeException("child failure ".$err);}$concurrent[]=json_decode($text,true);}}'+
 '[$status,$body]=mmRenewMetadata($root,$c->method??"GET",$c->query??"",$c->uri??"/catalog/compositions.v1.json");'+
 '$out[]=["name"=>$c->name,"status"=>$status,"body"=>$body,"pair"=>file_get_contents($root."/private/pair.json"),'+
 '"leaseExists"=>is_dir(dirname($root)."/.minimodel-deployment-lease.v1"),"journalExists"=>file_exists($root."/private/pending.json"),"nextExists"=>file_exists($root."/private/pair.next"),"concurrent"=>$concurrent];if(is_resource($held)){flock($held,LOCK_UN);fclose($held);}}'+
 '$policies=[];foreach(json_decode(file_get_contents("/renewal-tests/policies.json")) as $p){try{mmRenewReferences($p->entry,$p->refs);$ok=true;}catch(Throwable $e){$ok=false;}$policies[]=["name"=>$p->name,"ok"=>$ok];}'+
 '$out[]=["name"=>"reference-policy","policies"=>$policies];'+
 'echo json_encode($out,JSON_THROW_ON_ERROR),"\\n";';
 const r=spawnSync("docker",["run","--rm","-i","--pull=never","--network","none","--read-only","--cap-drop","ALL","--security-opt","no-new-privileges",
 "--tmpfs","/renewal-runtime:rw,noexec,nosuid,size=16777216",
 "--mount","type=bind,src="+root+",dst=/source,readonly","--mount","type=bind,src="+owned.rootPath+",dst=/renewal-tests",image.stdout.trim(),"php"],
 {input:php,encoding:"utf8",timeout:90000,maxBuffer:4*1024*1024,windowsHide:true});
 assert.equal(r.status,0,r.stdout+r.stderr);assert.equal(r.stderr,"");
 const output=JSON.parse(r.stdout),summaries=[];
 for(const actual of output){if(actual.name==="reference-policy"){assert.equal(actual.policies.length,policies.length);for(const p of actual.policies)assert.equal(p.ok,policies.find(x=>x.name===p.name).ok,p.name);continue;}const planned=cases.find(x=>x.name===actual.name);
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
 if(actual.name==="interrupted-next-retained")assert(actual.nextExists);
 if(actual.name==="concurrent-expired"){assert.equal(actual.concurrent.length,2);for(const child of actual.concurrent){assert.equal(child[0],200);assert.equal(JSON.parse(child[1]).revision,planned.initialRevision+1);}}
 summaries.push({name:actual.name,status:actual.status,renewed:planned.renew});
 }
 console.log(JSON.stringify({schema:"minimodel.host_renewal_local_test.v1",image:image.stdout.trim(),cases:summaries,referencePolicyCases:policies.length,productionKey:false,published:false}));
 }finally{removeOwnedTemporaryDirectory(owned);}
});
