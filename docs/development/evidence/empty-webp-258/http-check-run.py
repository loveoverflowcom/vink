from pathlib import Path
from html.parser import HTMLParser
import os,socket,subprocess,tempfile,time,signal,hashlib,json,urllib.request,urllib.error
root=Path('/workspace/scratch/429f9ca90843/vink-258-empty-webp'); pg=Path('/tmp/vink-235-pg/local/bin')
d=Path(tempfile.mkdtemp(prefix='vink258-http-')); app=root/'target/debug/vink-app'
def port():
 s=socket.socket(); s.bind(('127.0.0.1',0)); p=s.getsockname()[1]; s.close(); return p
p=port(); web=port(); base=f'http://127.0.0.1:{web}'
env={k:v for k,v in os.environ.items() if not k.startswith('VINK_') and k!='DATABASE_URL'}
env.update(VINK_PROFILE='local',VINK_BIND=f'127.0.0.1:{web}',VINK_PUBLIC_ORIGIN=base,VINK_SITE_ROOT=str(root/'target/site'),VINK_AUTH_SIGN_IN='disabled',VINK_LOG_FORMAT='text',VINK_DEV_PRINCIPAL='11111111-1111-4111-8111-111111111111')
def request(path):
 try:
  with urllib.request.urlopen(base+path, timeout=10) as r: return r.status,r.headers,r.read()
 except urllib.error.HTTPError as r: return r.code,r.headers,r.read()
def sql(url,text): subprocess.run([str(pg/'psql'),url,'-Xq','-v','ON_ERROR_STOP=1'],input=text.encode(),check=True,stdout=subprocess.DEVNULL)
def command(args, variables):
 with (d/'commands.log').open('ab') as f: subprocess.run([str(app),*args],cwd=root,env=variables,stdout=f,stderr=subprocess.STDOUT,check=True)
class Region(HTMLParser):
 def __init__(self): super().__init__(); self.depth=0; self.tags=[]; self.words=[]
 def handle_starttag(self,tag,attrs):
  a=dict(attrs)
  if tag=='section' and a.get('id')=='community-stream': self.depth=1
  elif self.depth and tag=='section': self.depth+=1
  if self.depth: self.tags.append((tag,a))
 def handle_endtag(self,tag):
  if tag=='section' and self.depth: self.depth-=1
 def handle_data(self,data):
  if self.depth: self.words.append(data)
def parse(raw): r=Region(); r.feed(raw.decode()); return r
with (d/'init.log').open('w') as f: subprocess.run([str(pg/'initdb'),'-D',str(d/'data'),'-A','trust','--encoding=UTF8','--locale=C.UTF-8'],stdout=f,stderr=subprocess.STDOUT,check=True)
pglog=(d/'pg.log').open('w'); proc=subprocess.Popen([str(pg/'postgres'),'-D',str(d/'data'),'-h','127.0.0.1','-p',str(p),'-k',''],stdout=pglog,stderr=subprocess.STDOUT); server=None
report={'boundary':'real PostgreSQL 17.11 + actual Axum/Leptos SSR and static pipeline; no browser interaction','checks':[]}
try:
 admin=f'postgresql://agent@127.0.0.1:{p}/postgres'
 for i in range(100):
  if subprocess.run([str(pg/'psql'),admin,'-Atc','select 1'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0: break
  time.sleep(.1)
 else: raise RuntimeError('PG startup failed')
 sql(admin,'CREATE ROLE vink258_http_runtime LOGIN; CREATE DATABASE vink258_http;')
 migrate=f'postgresql://agent@127.0.0.1:{p}/vink258_http'
 command(['migrate'],dict(env,VINK_MIGRATE_DATABASE_URL=migrate))
 sql(migrate,'REVOKE CREATE ON SCHEMA public FROM PUBLIC; GRANT USAGE ON SCHEMA public TO vink258_http_runtime; GRANT SELECT,INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA public TO vink258_http_runtime; GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO vink258_http_runtime; REVOKE INSERT,UPDATE,DELETE ON _sqlx_migrations FROM vink258_http_runtime;')
 env['DATABASE_URL']=f'postgresql://vink258_http_runtime@127.0.0.1:{p}/vink258_http'
 slog=(d/'app.log').open('w'); server=subprocess.Popen([str(app),'serve'],cwd=root,env=env,stdout=slog,stderr=subprocess.STDOUT)
 for i in range(100):
  try:
   if request('/health/ready')[0]==200: break
  except (OSError,urllib.error.URLError): pass
  time.sleep(.1)
 else: raise RuntimeError('App readiness failed')
 for path,code,art,words in [('/',200,'no-public-works','Chưa có tác phẩm công khai.'),('/?q=no-match-258',200,'no-results','Chưa có tác phẩm phù hợp.'),('/the/unknown-258',404,None,'Không thể tìm tác phẩm theo thẻ này.'),('/?cursor=broken',422,None,'Trang kết quả không còn hợp lệ')]:
  status,headers,raw=request(path); r=parse(raw); text=''.join(r.words); images=[a for t,a in r.tags if t=='img']; statuses=[a for t,a in r.tags if a.get('role')=='status']
  assert status==code,(path,status); assert words in text,(path,text)
  if art:
   assert len(images)==1 and images[0]['src']==f'/pkg/empty/vink-empty-{art}.webp'; assert images[0].get('alt')==''
   assert len(statuses)==1 and not any(t in ['h2','h3'] for t,a in r.tags)
   assert 'Trang này: 0' not in text
  else: assert not images
  if path.startswith('/?q='): assert any(a.get('id')=='clear-public-results' and a.get('href')=='/' for t,a in r.tags)
  report['checks'].append({'route':path,'status':status,'empty_images':len(images),'result_statuses':len(statuses),'result':'PASS'})
  (d/('page-'+str(len(report['checks']))+'.html')).write_bytes(raw)
 manifest=json.loads((root/'apps/vink-web/assets/empty/asset-manifest.json').read_text())
 assert sorted(x.name for x in (root/'target/site/pkg/empty').iterdir())==sorted(Path(a['file']).name for a in manifest['runtime_assets'])
 for a in manifest['runtime_assets']:
  path='/pkg/empty/'+Path(a['file']).name; status,headers,raw=request(path)
  assert status==200 and headers.get_content_type()=='image/webp' and len(raw)==a['bytes'] and hashlib.sha256(raw).hexdigest()==a['sha256']
  report['checks'].append({'route':path,'bytes':len(raw),'mime':headers.get_content_type(),'sha256':a['sha256'],'result':'PASS'})
 command(['dev-seed'],env)
 status,headers,raw=request('/'); r=parse(raw); assert status==200 and not any(t=='img' and a.get('src','').startswith('/pkg/empty/') for t,a in r.tags); assert 'Trang này: 1 tác phẩm' in ''.join(r.words)
 report['checks'].append({'route':'/ after real dev-seed','nonempty_count':1,'empty_images':0,'result':'PASS'})
 report['result']='PASS'; report['scratch_evidence']=str(d)
 Path('/tmp/vink-258-http-report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n'); print(json.dumps(report,ensure_ascii=False,indent=2),flush=True); print('APP_BASE='+base,flush=True); input('Holding task-owned review server; send newline to stop.\n')
finally:
 for child in [server,proc]:
  if child is not None:
   child.send_signal(signal.SIGTERM)
   try: child.wait(timeout=15)
   except subprocess.TimeoutExpired: child.kill(); child.wait()
 pglog.close()
