const params=new URLSearchParams(location.search);
if(params.has('manifest')){
 document.body.innerHTML='<pre id="manifest"></pre>';
 document.getElementById('manifest').textContent=JSON.stringify(screens.map(({render,...s})=>s),null,2);
}else{
 const selected=screens.find(s=>s.id===params.get('screen'))||screens[0];
 document.title=`${selected.product==='admin'?'管理端':'钱包'} · ${selected.title}`;
 document.getElementById('app').innerHTML=selected.render();
 document.documentElement.dataset.screen=selected.id;
 document.fonts.ready.then(()=>{document.documentElement.dataset.ready='true'});
}
window.designScreens=screens;
