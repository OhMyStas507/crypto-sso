import fs from 'node:fs/promises';
import path from 'node:path';
import http from 'node:http';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const dir=path.dirname(fileURLToPath(import.meta.url));
const repo=path.resolve(dir,'../..');
const bundled=process.env.GUI_NODE_MODULES||'C:/Users/32161/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules';
const require=createRequire(import.meta.url);
const {chromium}=require(path.join(bundled,'playwright'));
const sharp=require(path.join(bundled,'sharp'));
const mime={'.html':'text/html; charset=utf-8','.js':'text/javascript; charset=utf-8','.css':'text/css; charset=utf-8','.svg':'image/svg+xml','.png':'image/png','.json':'application/json; charset=utf-8'};
const server=http.createServer(async(req,res)=>{
 try{
  const pathname=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
  const target=path.resolve(dir,'.'+(pathname==='/'?'/index.html':pathname));
  if(!target.startsWith(dir+path.sep)){res.writeHead(403).end();return;}
  const bytes=await fs.readFile(target);
  res.writeHead(200,{'Content-Type':mime[path.extname(target)]||'application/octet-stream'}).end(bytes);
 }catch{res.writeHead(404).end();}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const base=`http://127.0.0.1:${server.address().port}`;
const browser=await chromium.launch({channel:'msedge',headless:true});
const failures=[];
try{
 const context=await browser.newContext({viewport:{width:1440,height:1000},deviceScaleFactor:2});
 const page=await context.newPage();
 page.on('pageerror',e=>failures.push({type:'script',message:e.message}));
 await page.goto(base+'/screen.html?manifest=1');
 const manifest=await page.evaluate(()=>window.designScreens.map(({render,...s})=>s));
 const icons=await page.evaluate(()=>[...new Set(window.designScreens.flatMap(s=>[...s.render().matchAll(/data-icon="([^"]+)"/g)].map(m=>m[1])))]);
 await fs.mkdir(path.join(dir,'assets/icons'),{recursive:true});
 for(const icon of icons)await fs.copyFile(path.join(repo,'wallet/node_modules/bootstrap-icons/icons',icon+'.svg'),path.join(dir,'assets/icons',icon+'.svg'));
 await fs.copyFile(path.join(repo,'wallet/node_modules/bootstrap-icons/LICENSE'),path.join(dir,'assets/icons/LICENSE'));
 await fs.mkdir(path.join(dir,'png/admin'),{recursive:true});
 await fs.mkdir(path.join(dir,'png/wallet'),{recursive:true});
 const specification=await fs.readFile(path.join(repo,'docs/identity-center-wallet-project-structure.md'),'utf8');
 const expectedRoutes=specification.split(/\r?\n/).filter(line=>line.startsWith('| `/')).flatMap(line=>[...line.split('|')[1].matchAll(/`([^`]+)`/g)].map(m=>m[1]));
 const coveredRoutes=new Set(manifest.map(s=>s.route));
 const missingRoutes=expectedRoutes.filter(route=>!coveredRoutes.has(route));
 const routeCoverage={document:'docs/identity-center-wallet-project-structure.md',required:expectedRoutes.length,covered:expectedRoutes.length-missingRoutes.length,missing:missingRoutes};
 if(missingRoutes.length)failures.push({type:'route-coverage',missing:missingRoutes});
 const metrics=[];
 for(const s of manifest.sort((a,b)=>a.id.localeCompare(b.id))){
  const size=s.product==='admin'?{width:1440,height:1000}:{width:1280,height:900};
  await page.setViewportSize(size);
  await page.goto(base+'/screen.html?screen='+s.id,{waitUntil:'networkidle'});
  await page.waitForFunction(()=>document.documentElement.dataset.ready==='true');
  const qa=await page.evaluate(()=>({
   overflow:[...document.querySelectorAll('.content,.modal,.auth')].filter(e=>e.scrollHeight>e.clientHeight+2||e.scrollWidth>e.clientWidth+2).map(e=>({class:e.className,height:e.clientHeight,scrollHeight:e.scrollHeight,width:e.clientWidth,scrollWidth:e.scrollWidth})),
   missingIcons:[...document.images].filter(e=>!e.complete||e.naturalWidth===0).map(e=>e.src),
   clippedControls:[...document.querySelectorAll('.btn,.input,th,td')].filter(e=>e.scrollWidth>e.clientWidth+2).map(e=>({class:e.className,text:e.textContent.slice(0,80)})),
   imageCount:document.images.length,
   viewport:{width:innerWidth,height:innerHeight}
  }));
  if(qa.overflow.length||qa.missingIcons.length||qa.clippedControls.length)failures.push({id:s.id,...qa});
  const file=`png/${s.product}/${s.id}.png`;
  await page.screenshot({path:path.join(dir,file),animations:'disabled'});
  s.file=file;s.width=size.width*2;s.height=size.height*2;
  metrics.push({id:s.id,...qa});
  console.log(`${s.id} ${s.title}${qa.overflow.length?' OVERFLOW':''}`);
 }
 await fs.writeFile(path.join(dir,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');
 await fs.writeFile(path.join(dir,'qa-report.json'),JSON.stringify({generatedAt:new Date().toISOString(),total:manifest.length,bootstrapIconsVersion:'1.13.2',routeCoverage,failures,metrics},null,2)+'\n');
 const escape=(s)=>s.replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
 const cards=(product)=>manifest.filter(s=>s.product===product).map(s=>`<article data-title="${escape(s.title+s.state)}"><a href="${s.file}" target="_blank"><img loading="lazy" src="${s.file}" alt="${escape(s.title)}"></a><div class="caption"><span><strong>${s.id} · ${escape(s.title)}</strong><small>${escape(s.state)}</small></span><span class="links"><a href="screen.html?screen=${s.id}">查看界面</a><a href="${s.file}" download>PNG ↓</a></span></div><code>${escape(s.route)}</code></article>`).join('');
 await fs.writeFile(path.join(dir,'index.html'),`<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Crypto SSO · GUI 设计图集</title><style>*{box-sizing:border-box}body{margin:0;background:#fafafa;color:#292930;font:13px 'Segoe UI','Microsoft YaHei',sans-serif}header{padding:28px 40px;border-bottom:1px solid #e4e4e9;background:white;position:sticky;top:0;z-index:1;display:flex;align-items:center;gap:25px}h1{font-size:19px;font-weight:600;margin:0}a{color:#665dc1;text-decoration:none}header span{color:#999;font-size:12px;margin-left:auto}input{width:220px;padding:9px;border:1px solid #ddd;border-radius:5px;font:inherit}main{padding:12px 40px 50px;max-width:1920px;margin:auto}h2{font-size:16px;font-weight:550;margin:30px 0 20px}.grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:25px}article{border:1px solid #dddde5;border-radius:7px;overflow:hidden;background:white}article img{display:block;width:100%;border-bottom:1px solid #eee}article .caption{padding:15px 17px 10px;display:flex;align-items:center;justify-content:space-between;gap:10px}strong{font-size:12px;font-weight:550}small{display:block;color:#999;font-size:10px;margin-top:7px}.links{display:flex;gap:15px;white-space:nowrap;font-size:11px}code{display:block;padding:0 17px 16px;color:#999;font-size:10px}footer{margin-top:30px;color:#999;font-size:11px;line-height:22px}@media(max-width:800px){.grid{grid-template-columns:1fr}header{padding:20px;flex-wrap:wrap}main{padding:10px 20px}.links{gap:9px}}</style><header><h1>Crypto SSO · GUI 设计图集</h1><a href="#admin">管理端 · ${manifest.filter(s=>s.product==='admin').length}</a><a href="#wallet">钱包 · ${manifest.filter(s=>s.product==='wallet').length}</a><span>${manifest.length} 张设计图 · PNG 2×</span><input id="q" placeholder="搜索页面或状态"></header><main><h2 id="admin">身份中心管理端</h2><div class="grid">${cards('admin')}</div><h2 id="wallet">本地身份钱包</h2><div class="grid">${cards('wallet')}</div><footer>根据业务流程与项目结构文档绘制。统一浅色设计，使用 Bootstrap Icons 1.13.2。<br>此图集为静态设计；示例数据表示不同业务阶段，接口与密码学消息为视觉示意。</footer></main><script>document.querySelector('#q').addEventListener('input',e=>document.querySelectorAll('article').forEach(a=>a.hidden=!a.dataset.title.includes(e.target.value)))</script></html>`);
 const thumbW=432,thumbH=300,labelH=46,margin=24,cols=3;
 for(const product of ['admin','wallet']){
  const items=manifest.filter(s=>s.product===product);
  const width=margin+(thumbW+margin)*cols;
  const height=84+Math.ceil(items.length/cols)*(thumbH+labelH+margin)+margin;
  const composites=[];
  const header=Buffer.from(`<svg width="${width}" height="70"><rect width="100%" height="100%" fill="#fafafa"/><text x="24" y="42" fill="#34343e" font-family="Segoe UI,Microsoft YaHei" font-size="22">Crypto SSO · ${product==='admin'?'身份中心管理端':'本地身份钱包'} / ${items.length} screens</text></svg>`);
  composites.push({input:header,left:0,top:0});
  for(let i=0;i<items.length;i++){
   const s=items[i],x=margin+(i%cols)*(thumbW+margin),y=84+Math.floor(i/cols)*(thumbH+labelH+margin);
   composites.push({input:await sharp(path.join(dir,s.file)).resize(thumbW,thumbH,{fit:'contain',background:'#fff'}).png().toBuffer(),left:x,top:y});
   const label=Buffer.from(`<svg width="${thumbW}" height="${labelH}"><rect width="100%" height="100%" fill="#fafafa"/><text x="0" y="21" fill="#52525e" font-family="Segoe UI,Microsoft YaHei" font-size="12">${s.id} · ${escape(s.title)}</text><text x="0" y="38" fill="#92929c" font-family="Segoe UI,Microsoft YaHei" font-size="10">${escape(s.state)}</text></svg>`);
   composites.push({input:label,left:x,top:y+thumbH});
  }
  await sharp({create:{width,height,channels:3,background:'#fafafa'}}).composite(composites).png().toFile(path.join(dir,`${product}-overview.png`));
 }
 // A compact pair for chat review; the full PNG files remain at native 2× resolution.
 const previews=[];
 for(const [i,id]of ['a04-review','w16-disclosure'].entries()){
  const s=manifest.find(s=>s.id===id);
  previews.push({input:await sharp(path.join(dir,s.file)).resize(1080,750,{fit:'contain',background:'#fff'}).png().toBuffer(),left:0,top:i*774});
 }
 await sharp({create:{width:1080,height:1524,channels:3,background:'#eeeef2'}}).composite(previews).png().toFile(path.join(dir,'preview.png'));
 console.log(JSON.stringify({total:manifest.length,icons:icons.length,routeCoverage,failures},null,2));
 if(failures.length)process.exitCode=1;
}finally{await browser.close();await new Promise(resolve=>server.close(resolve));}
