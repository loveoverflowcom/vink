from pathlib import Path
import os, shutil, subprocess, urllib.request, time, signal, hashlib, json
root=Path('/tmp/vink-220-review');out=Path('/tmp/vink-220-focused')
src=root/'apps/vink-web/e2e';dst=out/'e2e';shutil.copytree(src,dst,dirs_exist_ok=True)
p=dst/'browser-smoke.mjs';s=p.read_text()
extra='''else if (args.phase === 'appearance-editor-focused') {
    await page.viewport(1280);
    await page.goto(`${args.base}/ban-viet`);
    await page.waitFor(`document.documentElement.dataset.hydrated === 'true' && document.querySelector('#create-draft')`, 'focused editor setup');
    await page.click('#create-draft');
    await page.waitFor(`${EDITOR_READY}`, 'focused editor created');
    await page.typeAtEnd('input[name=title]', TITLE);
    await page.typeInBody(BODY + TAIL);
    await page.click('#save');
    await page.waitStatus('saved', 'focused saved baseline');
    await page.typeInBody(STANZA);
    const written = BODY + TAIL + STANZA;
    check((await page.body()) === written, 'focused unsaved Vietnamese stanza');
    await page.eval(`document.querySelector('.cm-editor').__vinkView = 'kept'`);
    const view = `document.querySelector('.cm-editor')?.__vinkView === 'kept' && document.querySelectorAll('.cm-editor').length === 1`;
    await page.lineStart(SHIFT);
    const selected = `getSelection().toString()`;
    check((await page.eval(selected)) === LAST_LINE, 'focused native selection ready');
    const editorPath = await page.eval(`location.pathname`);
    await shellWithDirtyEditor(page, editorPath, written, view, selected);
  }
  '''
needle="else if (args.phase === 'main') await mainPhase(page);"
assert needle in s
p.write_text(s.replace(needle,extra+needle).replace('../../../packages/ui-system/leptos/preview/e2e/select-geometry.mjs', (root/'packages/ui-system/leptos/preview/e2e/select-geometry.mjs').as_uri()))
a=s.index('async function shellWithDirtyEditor(');b=s.index('// State anatomy',a)
(out/'provenance.json').write_text(json.dumps({'source_sha':'17d6344e367e9816025352b262938dc6fe92c1ef','helper':'shellWithDirtyEditor','helper_sha256':hashlib.sha256(s[a:b].encode()).hexdigest(),'diagnostic_script_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'canonical_main_modified':False},indent=2)+'\n')
(out/'focused-dispatch.txt').write_text(extra)
def run(args,env=None): subprocess.run(args,cwd=root,env=env,check=True)
admin='postgresql://vink220@127.0.0.1:55520/postgres'
if not subprocess.check_output(['psql',admin,'-tAc',"SELECT 1 FROM pg_roles WHERE rolname='vink220_app'"],text=True).strip():
 run(['psql',admin,'-v','ON_ERROR_STOP=1','-c','CREATE ROLE vink220_app LOGIN'])
if not subprocess.check_output(['psql',admin,'-tAc',"SELECT 1 FROM pg_database WHERE datname='vink_220_focus'"],text=True).strip():
 run(['createdb','-h','127.0.0.1','-p','55520','-U','vink220','vink_220_focus'])
env=os.environ.copy();env.update(DATABASE_URL='postgresql://vink220_app@127.0.0.1:55520/vink_220_focus',VINK_MIGRATE_DATABASE_URL='postgresql://vink220@127.0.0.1:55520/vink_220_focus',VINK_PROFILE='local',VINK_BIND='127.0.0.1:55521',VINK_PUBLIC_ORIGIN='http://127.0.0.1:55521',VINK_SITE_ROOT=str(root/'target/site'),VINK_DEV_PRINCIPAL='11111111-1111-4111-8111-111111111111',VINK_AUTH_SIGN_IN='disabled',VINK_LOG_FORMAT='text')
run([str(root/'target/debug/vink-app'),'migrate'],env)
run(['psql',env['VINK_MIGRATE_DATABASE_URL'],'-v','ON_ERROR_STOP=1','-c','REVOKE CREATE ON SCHEMA public FROM PUBLIC; GRANT USAGE ON SCHEMA public TO vink220_app; GRANT SELECT,INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA public TO vink220_app; GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO vink220_app; REVOKE INSERT,UPDATE,DELETE ON _sqlx_migrations FROM vink220_app;'])
run([str(root/'target/debug/vink-app'),'dev-seed'],env)
with (out/'server.log').open('w') as log:
 server=subprocess.Popen([str(root/'target/debug/vink-app'),'serve'],cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT)
 try:
  for _ in range(100):
   try:
    urllib.request.urlopen('http://127.0.0.1:55521/health/live',timeout=1);break
   except Exception:time.sleep(.1)
  else:raise RuntimeError('local server not ready')
  run(['node',str(p),'--base','http://127.0.0.1:55521','--chrome','/Applications/Google Chrome.app/Contents/MacOS/Google Chrome','--state',str(out/'smoke-state.json'),'--phase','appearance-editor-focused'])
 finally:
  server.send_signal(signal.SIGTERM)
  try:server.wait(timeout=15)
  except subprocess.TimeoutExpired:server.kill();server.wait()
