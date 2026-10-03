//! A standalone viewer for the shared graph schema. Data never becomes markup.
use super::{Graph, Node};
use serde::Serialize;
use std::collections::BTreeMap;

fn source_href(node: &Node) -> Option<String> {
    if node
        .module
        .as_deref()
        .is_some_and(|module| module.starts_with("stdlib:"))
    {
        return None;
    }
    let source = node.source.as_ref()?;
    if source.line == 0 || source.column == Some(0) || source.file.contains('\0') {
        return None;
    }
    let path = source.file.replace('\\', "/");
    let bytes = path.as_bytes();
    let drive = bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1..3] == *b":/";
    if !path.starts_with('/') && !drive {
        return None;
    }
    let mut encoded = String::new();
    for (index, byte) in bytes.iter().copied().enumerate() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) || drive && index == 1 {
            encoded.push(byte as char);
        } else {
            use std::fmt::Write;
            write!(encoded, "%{byte:02X}").unwrap();
        }
    }
    let mut href = format!(
        "vscode://file/{}:{}",
        encoded.strip_prefix('/').unwrap_or(&encoded),
        source.line
    );
    if let Some(column) = source.column {
        href.push_str(&format!(":{column}"));
    }
    Some(href)
}

fn script_json(value: &impl Serialize) -> String {
    let json = serde_json::to_string(value).expect("graph fields serialize as JSON");
    let mut escaped = String::with_capacity(json.len());
    for character in json.chars() {
        match character {
            '<' => escaped.push_str("\\u003c"),
            '>' => escaped.push_str("\\u003e"),
            '&' => escaped.push_str("\\u0026"),
            '\u{2028}' => escaped.push_str("\\u2028"),
            '\u{2029}' => escaped.push_str("\\u2029"),
            _ => escaped.push(character),
        }
    }
    escaped
}

pub fn html(graph: &Graph) -> String {
    #[derive(Serialize)]
    struct Payload<'a> {
        graph: &'a Graph,
        source_links: BTreeMap<&'a str, String>,
    }
    let payload = Payload {
        graph,
        source_links: graph
            .nodes
            .iter()
            .filter_map(|node| source_href(node).map(|href| (node.id.as_str(), href)))
            .collect(),
    };
    // Concatenate once; data containing a template marker must remain data.
    let mut page = String::from(HEADER);
    page.push_str(&script_json(&payload));
    page.push_str(VIEWER);
    page
}

const HEADER: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="referrer" content="no-referrer">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'none'; img-src 'none'; font-src 'none'; base-uri 'none'; form-action 'none'">
<title>Nagi graph</title>
<style>
:root{color-scheme:light;--ink:#172b3a;--muted:#587080;--line:#dce6ed;--paper:#fff;--canvas:#f4f7fa;--accent:#145c83}
*{box-sizing:border-box}body{margin:0;height:100dvh;display:flex;flex-direction:column;font:14px/1.5 system-ui,sans-serif;color:var(--ink);background:var(--canvas)}
button,input,select{font:inherit}button{cursor:pointer;border:1px solid var(--line);border-radius:7px;padding:6px 10px;color:var(--ink);background:var(--paper)}button:hover{border-color:var(--accent);background:#edf6fb}button:focus-visible,input:focus-visible,select:focus-visible,a:focus-visible{outline:3px solid #82c7ef;outline-offset:2px}button:disabled{opacity:.5;cursor:default}
header{min-height:68px;flex-shrink:0;display:flex;align-items:center;justify-content:space-between;gap:16px;padding:12px 22px;background:var(--paper);border-bottom:1px solid var(--line)}h1{font-size:20px;margin:0;letter-spacing:-.4px}.subtitle{font-size:12px;color:var(--muted)}.toolbar{display:flex;gap:7px;align-items:center;flex-wrap:wrap}.toolbar span{min-width:42px;text-align:center;color:var(--muted);font-size:12px}
main{display:grid;flex:1;min-height:0;grid-template-columns:290px minmax(0,1fr)}aside{overflow:auto;background:var(--paper);border-right:1px solid var(--line);padding:18px}aside section+section{border-top:1px solid var(--line);padding-top:17px;margin-top:17px}h2{font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:var(--muted);margin:0 0 10px}p{margin:0 0 10px}.hint{font-size:12px;color:var(--muted)}.field{display:block;margin:10px 0}.field>span{display:block;font-size:12px;color:var(--muted);margin-bottom:4px}input[type=search],select{width:100%;min-width:0;border:1px solid var(--line);border-radius:6px;background:white;padding:7px;color:var(--ink)}.choices{display:flex;flex-wrap:wrap;gap:5px 12px}.choices label{font-size:12px;display:flex;align-items:center;gap:4px}.swatch{width:9px;height:9px;display:inline-block;border-radius:3px;flex-shrink:0}.group-row{display:flex;gap:8px;align-items:center;justify-content:space-between;margin:7px 0}.group-row span{font-size:12px;overflow-wrap:anywhere;min-width:0}.group-row button{font-size:11px;padding:3px 7px;flex-shrink:0}.pair{display:flex;gap:6px}.pair button{flex:1;font-size:12px}.warnings{background:#fff7e4;border:1px solid #edcf8e;border-radius:8px;padding:10px;font-size:12px}.warnings h2{color:#865c14}.warnings ul{padding-left:16px;margin:0;overflow-wrap:anywhere}.legend{display:flex;gap:6px 12px;flex-wrap:wrap;font-size:12px}
#canvas{position:relative;min-width:0;min-height:0;overflow:auto;background-image:radial-gradient(#dbe5ec 1px,transparent 1px);background-size:20px 20px}svg{display:block}.empty{padding:40px;font-size:15px;fill:var(--muted)}.group-frame{fill:#fff;fill-opacity:.82;stroke:var(--line);stroke-width:1}.group-heading{font-size:12px;font-weight:600;fill:var(--muted)}.edge{fill:none;stroke-width:1.5;opacity:.65;cursor:pointer}.edge:hover{stroke-width:3;opacity:1}.edge-label{font-size:10px;fill:var(--muted);paint-order:stroke;stroke:#fff;stroke-width:4px;stroke-linejoin:round;pointer-events:none}.card{cursor:pointer;outline:none}.card rect{fill:white;stroke:#cedce5;stroke-width:1.2;rx:9}.card:hover rect,.card:focus rect{stroke:var(--accent);stroke-width:2}.card.selected rect{stroke:var(--accent);stroke-width:2.5;fill:#edf7fc}.node-title{font-size:13px;font-weight:600;fill:var(--ink)}.node-signature{font:11px ui-monospace,monospace;fill:var(--muted)}.node-kind{font-size:10px;fill:var(--muted)}.badge{font-size:11px;fill:var(--muted)}#details{overflow-wrap:anywhere}#details h3{font-size:15px;margin:0 0 8px}#details pre{font:12px/1.5 ui-monospace,monospace;white-space:pre-wrap;overflow-wrap:anywhere;background:var(--canvas);padding:10px;border-radius:7px}#details a{color:var(--accent);font-size:12px}#details ul{padding-left:16px;font-size:12px}#details li{margin:6px 0}.relationship{display:block;border:0;background:none;text-align:left;padding:0;color:var(--accent);font-size:12px;overflow-wrap:anywhere}#focus-selected{width:100%;margin-top:10px}.noscript{padding:20px;color:#865c14}
@media(max-width:760px){header{padding:10px 12px;align-items:flex-start;gap:8px}h1{font-size:18px}.toolbar{justify-content:flex-end;gap:4px}.toolbar button{padding:5px 8px}main{grid-template-columns:1fr;grid-template-rows:auto minmax(0,1fr)}aside{max-height:38dvh;border-right:0;border-bottom:1px solid var(--line);padding:12px}aside section{display:block}aside section+section{margin-top:12px;padding-top:12px}}
</style>
</head>
<body>
<header><div><h1>Nagi graph</h1><div id="summary" class="subtitle">Source relationships</div></div><div class="toolbar"><button id="zoom-out" aria-label="Zoom out">−</button><span id="zoom-label">100%</span><button id="zoom-in" aria-label="Zoom in">+</button><button id="fit">Fit</button><button id="reset">Reset</button><button id="download">JSON</button></div></header>
<main>
<aside>
<section id="warnings" class="warnings" hidden><h2>Warnings</h2><ul></ul></section>
<section><h2>Explore</h2><label class="field"><span>Find a node</span><input id="search" type="search" placeholder="Name or module" autocomplete="off"></label><label class="field"><span>Focus neighbourhood</span><select id="focus"><option value="">All nodes</option></select></label><label class="field"><span>Relationship hops</span><select id="depth"><option>1</option><option>2</option><option>3</option></select></label><div id="edge-filters" class="choices"></div></section>
<section><h2>Module groups</h2><div class="pair"><button id="expand">Expand all</button><button id="collapse">Collapse all</button></div><div id="groups"></div></section>
<section><h2>Node kinds</h2><div id="legend" class="legend"></div></section>
<section><h2>Details</h2><div id="details"><p class="hint">Select a node or relationship. Source links open in VS Code only when clicked.</p></div><button id="focus-selected" disabled>Focus selected node</button></section>
</aside>
<div id="canvas"><svg id="graph" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Nagi source relationships"></svg></div>
</main>
<noscript><p class="noscript">Enable JavaScript to explore this local graph. The file contains all graph data and uses no external assets.</p></noscript>
<script id="graph-data" type="application/json">"##;

const VIEWER: &str = r##"</script>
<script>
'use strict';
(() => {
 const payload=JSON.parse(document.getElementById('graph-data').textContent);
 const graph=payload.graph, links=payload.source_links, nodes=new Map(graph.nodes.map(n=>[n.id,n])), nodeIndexes=new Map(graph.nodes.map((n,index)=>[n.id,index]));
 const $=id=>document.getElementById(id), ns='http://www.w3.org/2000/svg';
 const colors={module:'#b7791f',type:'#64748b',class:'#7c3aed',enum:'#0d9488',function:'#2563eb'};
 const edgeColors={calls:'#2563eb',contains:'#b7791f',owns:'#7c3aed',borrows:'#15803d',shares:'#be185d',returns:'#0891b2',depends_on:'#b45309',uses:'#64748b'};
 const meanings={calls:'Direct statically resolved call',contains:'Declared membership',owns:'Owned type relationship',borrows:'Borrowed type relationship',shares:'Shared type relationship',returns:'Declared return type',depends_on:'Resolved module dependency',uses:'Declared type dependency'};
 const kindText=kind=>kind.replaceAll('_',' ');
 const text=(tag,value,cls)=>{const e=document.createElement(tag);e.textContent=value;if(cls)e.className=cls;return e;};
 const svg=(tag,attrs={},value)=>{const e=document.createElementNS(ns,tag);for(const [k,v] of Object.entries(attrs))e.setAttribute(k,String(v));if(value!==undefined)e.textContent=value;return e;};
 const short=(value,max)=>{const parts=Array.from(value);return parts.length>max?parts.slice(0,max-1).join('')+'…':value;};
 const color=kind=>colors[kind]||'#64748b';
 const kinds=[...new Set(graph.edges.map(e=>e.kind))].sort(), enabled=new Set(kinds);
 const membership=new Map(), groups=[];
 for(const g of graph.groups){const members=g.nodes.filter(id=>nodes.has(id)&&!membership.has(id));if(members.length){const entry={...g,nodes:members};groups.push(entry);for(const id of members)membership.set(id,entry);}}
 const others=graph.nodes.filter(n=>!membership.has(n.id)).map(n=>n.id);
 if(others.length){const g={id:null,label:'Other definitions',nodes:others};groups.push(g);for(const id of others)membership.set(id,g);}
 const relatedNames=new Map(),nameIndex=new Map();
 for(const node of nodes.values()){const name=node.label.split('\n')[0];relatedNames.set(node.id,name);if(!nameIndex.has(name))nameIndex.set(name,[]);nameIndex.get(name).push(node);}
 for(const [name,matches] of nameIndex){if(matches.length<2)continue;const candidates=new Map(),candidate=node=>name+' · '+membership.get(node.id).label;for(const node of matches){const label=candidate(node);candidates.set(label,(candidates.get(label)||0)+1);}for(const node of matches){const label=candidate(node);relatedNames.set(node.id,candidates.get(label)===1?label:node.qualified_name||node.id);}}
 const collapsed=new Set(graph.nodes.length>120?groups:[]), initialCollapsed=new Set(collapsed);
 let selected=null, focus=null, zoom=1, width=800,height=400;
 $('summary').textContent=`${graph.nodes.length} nodes · ${graph.edges.length} relationships · ${graph.groups.length} modules`;
 if(graph.warnings.length){$('warnings').hidden=false;for(const warning of graph.warnings)$('warnings').querySelector('ul').append(text('li',warning));}
 for(const kind of [...new Set(graph.nodes.map(n=>n.kind))].sort()){const item=text('span','');const dot=text('span','','swatch');dot.style.backgroundColor=color(kind);item.append(dot,document.createTextNode(' '+kindText(kind)));$('legend').append(item);}
 for(const kind of kinds){const label=text('label',''),check=document.createElement('input');check.type='checkbox';check.checked=true;check.addEventListener('change',()=>{check.checked?enabled.add(kind):enabled.delete(kind);render();});label.append(check,document.createTextNode(kindText(kind)));label.title=meanings[kind]||kindText(kind);$('edge-filters').append(label);}
 const groupButtons=new Map();
 for(const group of groups){const row=text('div','','group-row'),label=text('span',group.label),button=text('button','');button.addEventListener('click',()=>{collapsed.has(group)?collapsed.delete(group):collapsed.add(group);render();});row.append(label,button);$('groups').append(row);groupButtons.set(group,button);}
 function choices(){const query=$('search').value.trim().toLocaleLowerCase();$('focus').replaceChildren(text('option','All nodes'));$('focus').firstChild.value='';for(const node of graph.nodes){if(query&&!(node.label+' '+node.qualified_name).toLocaleLowerCase().includes(query))continue;const option=text('option',node.qualified_name||node.label);option.value=node.id;$('focus').append(option);}if(focus&&!Array.from($('focus').options).some(o=>o.value===focus)){const node=nodes.get(focus),option=text('option',node.qualified_name);option.value=focus;$('focus').append(option);}$('focus').value=focus||'';}
 function visibleIds(){if(!focus)return new Set(nodes.keys());const keep=new Set([focus]);let frontier=[focus];for(let hop=0;hop<Number($('depth').value);hop++){const next=[];for(const edge of graph.edges){if(!enabled.has(edge.kind))continue;if(frontier.includes(edge.from)&&nodes.has(edge.to)&&!keep.has(edge.to)){keep.add(edge.to);next.push(edge.to);}if(frontier.includes(edge.to)&&nodes.has(edge.from)&&!keep.has(edge.from)){keep.add(edge.from);next.push(edge.from);}}frontier=next;}return keep;}
 function detailNode(node){selected=node.id;const area=$('details');area.replaceChildren(text('h3',node.label.split('\n')[0]),text('p',kindText(node.kind),'hint'),text('pre',node.label),text('p',node.qualified_name,'hint'));if(node.source){const position=node.source.file+':'+node.source.line+(node.source.column?':'+node.source.column:'');area.append(text('p',position,'hint'));if(Object.hasOwn(links,node.id)){const link=text('a','Open source in VS Code');link.href=links[node.id];link.rel='noopener noreferrer';area.append(link);}}else area.append(text('p','No physical source location','hint'));const list=text('ul','');for(const edge of graph.edges.filter(e=>e.from===node.id||e.to===node.id)){const other=nodes.get(edge.from===node.id?edge.to:edge.from);if(!other)continue;const row=text('li',''),button=text('button',`${edge.from===node.id?'→':'←'} ${kindText(edge.kind)} · ${relatedNames.get(other.id)}`,'relationship');button.addEventListener('click',()=>{detailNode(other);render();});row.append(button);if(edge.label)row.append(text('span',edge.label,'hint'));list.append(row);}if(list.childNodes.length)area.append(list);$('focus-selected').disabled=false;}
 function detailGroup(group,members){selected=null;$('focus-selected').disabled=true;$('details').replaceChildren(text('h3',group.label),text('p',`${members.length} definitions in this group`,'hint'));const list=text('ul','');for(const node of members){const row=text('li',''),button=text('button',node.label.split('\n')[0],'relationship');button.addEventListener('click',()=>{collapsed.delete(group);detailNode(node);render();});row.append(button);list.append(row);}$('details').append(list);}
 function detailEdge(entry){selected=null;$('focus-selected').disabled=true;const area=$('details');area.replaceChildren(text('h3',kindText(entry.kind)),text('p',meanings[entry.kind]||kindText(entry.kind),'hint'));for(const edge of entry.edges){const from=nodes.get(edge.from),to=nodes.get(edge.to);area.append(text('p',`${from?from.qualified_name:edge.from} → ${to?to.qualified_name:edge.to}`));if(edge.label)area.append(text('pre',edge.label));}render();}
 const boxKey=box=>box.node?'n'+nodeIndexes.get(box.node.id):'g'+groups.indexOf(box.group);
 function render(){const drawing=$('graph'),active=document.activeElement,activeKey=drawing.contains(active)?active.getAttribute('data-focus-key'):null,focusTargets=new Map();for(const [group,button] of groupButtons)button.textContent=collapsed.has(group)?'Expand':'Collapse';const keep=visibleIds(),boxes=[],at=new Map(),frames=[];let top=28;for(const group of groups){const members=group.nodes.filter(id=>keep.has(id)).map(id=>nodes.get(id));if(!members.length)continue;const groupBoxes=[];if(collapsed.has(group)){const box={group,members,x:40,y:top+34,w:248,h:76};boxes.push(box);groupBoxes.push(box);for(const node of members)at.set(node.id,box);}else{const indegree=new Map(members.map(n=>[n.id,0])),adj=new Map(),levels=new Map(members.map(n=>[n.id,0]));for(const edge of graph.edges){if(enabled.has(edge.kind)&&indegree.has(edge.from)&&indegree.has(edge.to)&&edge.from!==edge.to){if(!adj.has(edge.from))adj.set(edge.from,new Set());if(!adj.get(edge.from).has(edge.to)){adj.get(edge.from).add(edge.to);indegree.set(edge.to,indegree.get(edge.to)+1);}}}const queue=members.filter(n=>indegree.get(n.id)===0).map(n=>n.id);for(let cursor=0;cursor<queue.length;cursor++){const id=queue[cursor];for(const next of adj.get(id)||[]){levels.set(next,Math.min(3,Math.max(levels.get(next),levels.get(id)+1)));indegree.set(next,indegree.get(next)-1);if(indegree.get(next)===0)queue.push(next);}}const rows=[0,0,0,0];for(const node of members){const level=levels.get(node.id),box={node,x:40+level*280,y:top+34+rows[level]++*112,w:248,h:92};boxes.push(box);groupBoxes.push(box);at.set(node.id,box);}}
 const right=Math.max(...groupBoxes.map(b=>b.x+b.w))+18,bottom=Math.max(...groupBoxes.map(b=>b.y+b.h))+18;frames.push({group,x:22,y:top,w:right-22,h:bottom-top});top=bottom+24;}
 width=Math.max(700,...frames.map(f=>f.x+f.w+24));height=Math.max(380,top);drawing.replaceChildren();drawing.setAttribute('viewBox',`0 0 ${width} ${height}`);const defs=svg('defs');for(const kind of kinds){const marker=svg('marker',{id:'arrow-'+kind,viewBox:'0 0 10 10',refX:9,refY:5,markerWidth:6,markerHeight:6,orient:'auto-start-reverse'});marker.append(svg('path',{d:'M 0 0 L 10 5 L 0 10 z',fill:edgeColors[kind]||'#64748b'}));defs.append(marker);}drawing.append(defs);
 for(const frame of frames){drawing.append(svg('rect',{x:frame.x,y:frame.y,width:frame.w,height:frame.h,rx:12,class:'group-frame'}));const title=svg('text',{x:frame.x+18,y:frame.y+23,class:'group-heading'},short(frame.group.label,Math.floor(frame.w/8)));title.append(svg('title',{},frame.group.label));drawing.append(title);}
 const aggregated=new Map();for(const edge of graph.edges){if(!enabled.has(edge.kind)||!at.has(edge.from)||!at.has(edge.to))continue;const from=at.get(edge.from),to=at.get(edge.to);if(from===to&&from.group)continue;const key=JSON.stringify([boxKey(from),boxKey(to),edge.kind]);if(!aggregated.has(key))aggregated.set(key,{from,to,kind:edge.kind,key,edges:[]});aggregated.get(key).edges.push(edge);}
 const labelBoxes=[],smallGraph=boxes.length<=40&&aggregated.size<=60;
 for(const entry of aggregated.values()){const a=entry.from,b=entry.to,self=a===b;const x1=a.x+a.w,y1=a.y+a.h/2,x2=self?x1:b.x,y2=self?y1+24:b.y+b.h/2;const bend=self?64:Math.max(40,Math.abs(x2-x1)/2),path=svg('path',{d:`M ${x1} ${y1} C ${x1+bend} ${y1}, ${x2-bend} ${y2}, ${x2} ${y2}`,class:'edge',stroke:edgeColors[entry.kind]||'#64748b','marker-end':`url(#arrow-${entry.kind})`,tabindex:0,role:'button','data-focus-key':'e'+entry.key,'aria-label':`${kindText(entry.kind)} relationship`});focusTargets.set('e'+entry.key,path);if(entry.kind==='borrows'||entry.kind==='shares')path.setAttribute('stroke-dasharray','5 4');path.append(svg('title',{},(meanings[entry.kind]||entry.kind)+(entry.edges[0].label?' · '+entry.edges[0].label:'')));path.addEventListener('click',()=>detailEdge(entry));path.addEventListener('keydown',event=>{if(event.key==='Enter'||event.key===' '){event.preventDefault();detailEdge(entry);}});drawing.append(path);if(!self&&(smallGraph||entry.edges.length>1)){const field=entry.edges[0].label?.split(':')[0].trim(),kind=kindText(entry.kind),full=entry.edges.length>1?`${kind} ×${entry.edges.length}`:kind+(field?' · '+short(field,12):'');for(const value of full===kind?[kind]:[full,kind]){const cx=(x1+x2)/2,cy=(y1+y2)/2-6,w=Array.from(value).length*5.5+6,bounds={x:cx-w/2,y:cy-10,w,h:14};const intersects=b=>bounds.x<b.x+b.w+3&&bounds.x+bounds.w>b.x-3&&bounds.y<b.y+b.h+3&&bounds.y+bounds.h>b.y-3;if(boxes.some(intersects)||labelBoxes.some(intersects))continue;const label=svg('text',{x:cx,y:cy,'text-anchor':'middle',class:'edge-label'},value);label.append(svg('title',{},entry.edges[0].label||kind));drawing.append(label);labelBoxes.push(bounds);break;}}}
 for(const box of boxes){const label=box.node?box.node.label:box.group.label,kind=box.node?box.node.kind:'module',card=svg('g',{class:'card'+(box.node&&box.node.id===selected?' selected':''),tabindex:0,role:'button','data-focus-key':boxKey(box),'aria-label':label});focusTargets.set(boxKey(box),card);card.append(svg('rect',{x:box.x,y:box.y,width:box.w,height:box.h}),svg('rect',{x:box.x,y:box.y+9,width:3,height:box.h-18,fill:color(kind),style:'fill:'+color(kind)+';stroke:none;rx:1'}),svg('text',{x:box.x+15,y:box.y+22,class:'node-title'},short(label.split('\n')[0],29)),svg('title',{},label));if(box.node){const lines=label.split('\n').slice(1,3);for(let i=0;i<lines.length;i++)card.append(svg('text',{x:box.x+15,y:box.y+41+i*14,class:'node-signature'},short(lines[i],35)));card.append(svg('text',{x:box.x+15,y:box.y+box.h-12,class:'node-kind'},kindText(kind)));}else card.append(svg('text',{x:box.x+15,y:box.y+48,class:'badge'},`${box.members.length} definitions · click to expand`));const activate=()=>{if(box.node)detailNode(box.node);else{collapsed.delete(box.group);detailGroup(box.group,box.members);}render();};card.addEventListener('click',activate);card.addEventListener('keydown',event=>{if(event.key==='Enter'||event.key===' '){event.preventDefault();activate();}});drawing.append(card);}
 if(!boxes.length)drawing.append(svg('text',{x:40,y:64,class:'empty'},'No nodes in this view. Reset the filters to explore the graph.'));scale();let restore=activeKey;if(restore?.startsWith('g')&&!focusTargets.has(restore)){const group=groups[Number(restore.slice(1))],first=boxes.find(box=>box.node&&membership.get(box.node.id)===group);if(first)restore=boxKey(first);}if(restore&&focusTargets.has(restore))focusTargets.get(restore).focus({preventScroll:true});}
 function scale(){$('graph').style.width=(width*zoom)+'px';$('graph').style.height=(height*zoom)+'px';$('zoom-label').textContent=Math.round(zoom*100)+'%';}
 $('search').addEventListener('input',choices);$('focus').addEventListener('change',()=>{focus=$('focus').value||null;if(focus){collapsed.delete(membership.get(focus));detailNode(nodes.get(focus));}render();});$('depth').addEventListener('change',render);
 $('focus-selected').addEventListener('click',()=>{if(selected){focus=selected;collapsed.delete(membership.get(focus));choices();render();}});
 $('expand').addEventListener('click',()=>{collapsed.clear();render();});$('collapse').addEventListener('click',()=>{for(const group of groups)collapsed.add(group);render();});
 $('zoom-in').addEventListener('click',()=>{zoom=Math.min(2,zoom*1.2);scale();});$('zoom-out').addEventListener('click',()=>{zoom=Math.max(.2,zoom/1.2);scale();});$('fit').addEventListener('click',()=>{zoom=Math.min(1,Math.max(.2,($('canvas').clientWidth-12)/width));scale();});
 $('reset').addEventListener('click',()=>{focus=null;selected=null;zoom=1;$('search').value='';$('depth').value='1';enabled.clear();for(const kind of kinds)enabled.add(kind);for(const input of $('edge-filters').querySelectorAll('input'))input.checked=true;collapsed.clear();for(const group of initialCollapsed)collapsed.add(group);$('details').replaceChildren(text('p','Select a node or relationship.','hint'));$('focus-selected').disabled=true;choices();render();});
 $('download').addEventListener('click',()=>{const blob=new Blob([JSON.stringify(graph,null,2)+'\n'],{type:'application/json'}),url=URL.createObjectURL(blob),link=document.createElement('a');link.href=url;link.download='nagi-graph.json';link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);});
 choices();render();
})();
</script>
</body>
</html>
"##;
