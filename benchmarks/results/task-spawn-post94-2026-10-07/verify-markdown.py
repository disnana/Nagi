from pathlib import Path
from collections import Counter
from urllib.parse import urlsplit,unquote
import json,re,subprocess,sys
from markdown_it import MarkdownIt
root=Path(__file__).resolve().parents[0]
repo=Path(__file__).resolve().parents[3]
files=set(subprocess.check_output(['git','ls-files','--','*.md'],cwd=repo,text=True).splitlines())
files.update(str(p.relative_to(repo)) for p in (repo/'docs').rglob('*.md'))
parser=MarkdownIt('commonmark')
slugs={}
def anchors(path):
    if path not in slugs:
        text=path.read_text(encoding='utf-8')
        ids=set(re.findall(r'(?:id|name)=[\"\']([^\"\']+)[\"\']',text));counts=Counter()
        tokens=parser.parse(text)
        for i,t in enumerate(tokens):
            if t.type!='heading_open': continue
            inline=tokens[i+1]
            title=''.join(c.content for c in inline.children or [] if c.type in ('text','code_inline'))
            slug=re.sub(r'[^\w\- ]','',title.lower()).replace(' ','-')
            n=counts[slug];counts[slug]+=1
            ids.add(slug+(f'-{n}' if n else ''))
        slugs[path]=ids
    return slugs[path]
checked=external=0;errors=[];count=0
for name in sorted(files):
    if name.startswith(('benchmarks/results/','build/')):continue
    path=repo/name
    if not path.is_file():continue
    count+=1
    for token in parser.parse(path.read_text(encoding='utf-8')):
        for child in token.children or []:
            if child.type not in ('link_open','image'):continue
            href=child.attrGet('href') if child.type=='link_open' else child.attrGet('src')
            if not href:continue
            url=urlsplit(href)
            if url.scheme or url.netloc:external+=1;continue
            target=(path.parent/unquote(url.path)).resolve() if url.path else path
            checked+=1
            if not target.exists():errors.append([name,href,'missing file'])
            elif url.fragment and target.suffix=='.md' and unquote(url.fragment) not in anchors(target):
                errors.append([name,href,'missing anchor'])
result=dict(markdown_files=count,local_links=checked,external_links_not_fetched=external,errors=errors)
print(json.dumps(result,ensure_ascii=False,indent=2))
(Path(sys.argv[1])).write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
assert not errors
