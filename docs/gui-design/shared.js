/* Static design source. All data is fictional. No business services are called. */
const screens=[];
const add=(id,product,title,route,render,state='默认')=>screens.push({id,product,title,route,render,state});
const I=(name,cls='')=>`<img class="icon ${cls}" src="assets/icons/${name}.svg" alt="" data-icon="${name}">`;
const B=(label,cls='',icon='')=>`<button class="btn ${cls}">${icon?I(icon):''}${label}</button>`;
const badge=(label,tone='',icon='circle')=>`<span class="badge ${tone}">${I(icon)}${label}</span>`;
const ck=(checked=false,locked=false)=>`<span class="check ${checked?'checked':''} ${locked?'locked':''}">${checked?I('check'):''}</span>`;
const sw=(on=true)=>`<span class="switch ${on?'on':''}"></span>`;
const inp=(value,cls='',icon='')=>`<div class="input ${cls}">${value}${icon?I(icon,'small'):''}</div>`;
const field=(label,value,hint='',cls='',icon='')=>`<div class="field"><label>${label}</label>${inp(value,cls,icon)}${hint?`<div class="hint">${hint}</div>`:''}</div>`;
const note=(content,tone='',icon='info-circle')=>`<div class="note ${tone}">${I(icon)}<span>${content}</span></div>`;
const notice=(content,tone='',action='',icon='info-circle')=>`<div class="notice ${tone}">${I(icon)}<span>${content}</span>${action}</div>`;
const kv=(rows)=>`<div class="kv">${rows.map(([k,v])=>`<div>${k}</div><div>${v}</div>`).join('')}</div>`;
const head=(title,actions='',meta='')=>`<div class="page-head"><div class="head-title"><h1>${title}</h1>${meta}</div><div class="actions">${actions}</div></div>`;
const panel=(title,content,meta='')=>`<div class="panel"><div class="panel-head">${title}<span class="muted tiny">${meta}</span></div><div class="panel-body">${content}</div></div>`;
const section=(title,content,meta='')=>`<section class="section"><h2 class="section-title">${title}<span class="meta">${meta}</span></h2>${content}</section>`;
const tabs=(items,index=0,cls='')=>`<div class="tabs ${cls}">${items.map((s,i)=>`<span class="tab ${i===index?'active':''}">${s}</span>`).join('')}</div>`;
const toolbar=(placeholder,filters=[],total='')=>`<div class="toolbar"><div class="search">${I('search')}${placeholder}</div>${filters.map(t=>B(t,'','chevron-down')).join('')}<span class="end tiny muted">${total}</span></div>`;
const table=(headers,rows,widths=[])=>`<table class="table"><colgroup>${headers.map((_,i)=>`<col${widths[i]?` style="width:${widths[i]}"`:''}>`).join('')}</colgroup><thead><tr>${headers.map(h=>`<th>${h}</th>`).join('')}</tr></thead><tbody>${rows.map((row,i)=>`<tr${i===0?' class="row-selected"':''}>${row.map(v=>`<td>${v}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
const paginate=(text='显示 1–8 条，共 24 条')=>`<div class="pagination"><span>${text}</span><div class="actions">${B('上一页','disabled','chevron-left')}${B('下一页','','chevron-right')}</div></div>`;
const steps=(labels,index)=>`<div class="steps">${labels.map((s,i)=>`${i?'<i class="connector"></i>':''}<span class="${i===index?'active':''}"><i class="step-num">${i<index?I('check','small'):i+1}</i>${s}</span>`).join('')}</div>`;
const timeline=(items)=>`<div class="timeline">${items.map(([title,time],i)=>`<div class="timeline-item ${i===0?'active':''}">${title}<div class="time">${time}</div></div>`).join('')}</div>`;
const modal=(title,body,footer)=>`<div class="modal-overlay"><div class="modal"><div class="modal-title">${title}${I('x-lg')}</div><div class="modal-body">${body}</div><div class="modal-footer">${footer}</div></div></div>`;
const stats=(items)=>`<div class="stat-strip">${items.map(([label,value,foot])=>`<div class="stat"><div class="label">${label}</div><div class="value">${value}</div><div class="foot">${foot}</div></div>`).join('')}</div>`;
const object=(icon)=>`<span class="object-icon">${I(icon)}</span>`;
const listrow=(icon,title,sub,right)=>`<div class="list-row"><div class="row-main">${object(icon)}<div>${title}${sub?`<div class="sub">${sub}</div>`:''}</div></div><div class="right-group">${right}</div></div>`;
const FP='2f87 6c3a 91be 804d 7c10 e542 a8f3 2d69 4ca0 571d 8e32 b96f c035 e7a4 19d2 6b80';
const FP2='82c4 119e a7d5 036b e2fa 6870 92d1 5bc8 d431 8f6a 0e29 75b0 1fa9 36d2 8bc4 570e';
const request='req_7d3a8f20c194';
const hash='94d17e06a8c352f0d43b9217e65a12c91ea0d7b34f25a89c60e2b4783d16f95a';
const claims=[['name','林知远'],['email','zhiyuan.lin@example.org'],['role','member']];
const claimsTable=(values=claims,source=false)=>`<div class="claim-table">${table(source?['属性键','申请值','建议差异']:['属性键','值'],values.map(([k,v],i)=>[`<span class="claim-key">${k}</span>`,v,...(source?[i===2?'<span class="tiny secondary">默认值 · 需核验</span>':'<span class="tiny muted">用户填写</span>']:[])]),source?['25%','43%','32%']:['32%','68%'])}</div>`;
const originCard=()=>`<div class="app-identity">${object('grid-1x2')}<div><strong>协作空间</strong><div class="origin mono">https://workspace.example.org</div></div>${badge('已确认来源','success','shield-check')}</div>`;
const formFooter=(primary='保存更改',other='取消')=>`<div class="form-footer">${B(other)}${B(primary,'primary')}</div>`;
const pref=(title,desc,control)=>`<div class="pref-row"><div>${title}${desc?`<div class="desc">${desc}</div>`:''}</div>${control}</div>`;

function shell(product,active,title,body,overlay='',parent=''){
 const admin=product==='admin';
 const groups=admin?[
  ['', [['grid-1x2','概览','overview'],['inbox','注册申请','enrollments','12'],['people','成员','members'],['file-earmark-check','签发台账','issuances','2']]],
  ['管理',[['sliders2','服务设置','service'],['list-ul','Claims 默认建议','claims'],['fingerprint','签发身份','issuer'],['clock-history','审计记录','audit']]]
 ]:[
  ['', [['grid-1x2','首页','home'],['person-vcard','我的凭证','credentials','2'],['inbox','注册申请','enrollments','1']]],
  ['连接',[['buildings','身份中心','centers'],['app-indicator','受信应用','apps']]]
 ];
 const nav=groups.map(([label,items])=>`${label?`<div class="nav-caption">${label}</div>`:''}${items.map(([icon,label,key,count])=>`<div class="nav-item ${active===key?'selected':''}">${I(icon)}<span>${label}</span>${count?`<span class="count">${count}</span>`:''}</div>`).join('')}`).join('');
 return `<div class="app ${admin?'admin':'wallet'}"><aside class="sidebar"><div class="brand"><span class="brandmark">${I(admin?'intersect':'shield-lock')}</span>${admin?'身份中心':'身份钱包'}${I('chevron-expand','chev')}</div>${nav}<div class="nav-spacer"></div>${!admin?`<div class="nav-item ${active==='settings'?'selected':''}">${I('gear')}<span>本地设置</span></div><div class="nav-item">${I('lock')}<span>锁定钱包</span></div>`:''}<div class="sidebar-bottom"><span class="avatar">${admin?'AD':'林'}</span><span>${admin?'管理员':'本地钱包'}<div class="muted" style="font-size:10px;margin-top:3px">${admin?'admin@example.org':'Windows 安全保护'}</div></span>${I(admin?'box-arrow-right':'shield-check')}</div></aside><main class="workspace"><div class="topbar">${I(admin?'buildings':'shield-lock')}<span class="crumb">${admin?'星河身份中心':'个人空间'}</span><span class="slash">/</span>${parent?`<span class="crumb">${parent}</span><span class="slash">/</span>`:''}<span>${title}</span><div class="top-right">${admin?'<span>2026年10月30日</span>':badge('已解锁','','unlock')}${I('question-circle')}${I(admin?'bell':'three-dots')}</div></div><div class="content">${body}</div>${overlay}</main></div>`;
}
function auth(product,title,body,icon=''){
 return `<div class="auth ${product==='wallet'?'wallet':''}"><div class="auth-brand"><span class="brandmark">${I(product==='admin'?'intersect':'shield-lock')}</span>${product==='admin'?'身份中心':'身份钱包'}</div><div class="auth-main">${icon?`<div class="hero-icon">${I(icon)}</div>`:''}<h1>${title}</h1>${body}</div><div class="auth-foot"><span>${product==='admin'?'星河身份中心':'本地数据由 Windows 安全存储保护'}</span><span>Crypto SSO · v0.1</span></div></div>`;
}
