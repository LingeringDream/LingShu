import argparse, functools, http.server, json, os, shutil, socket, subprocess, tempfile, threading, time, urllib.request, urllib.error
from pathlib import Path

parser=argparse.ArgumentParser(description='Real CLI installer/MCP/upgrade/uninstall regression (includes PostgreSQL)')
parser.add_argument('--archive', type=Path, required=True)
args=parser.parse_args()
root=Path(__file__).resolve().parent.parent
work=tempfile.TemporaryDirectory(prefix='lingshu lifecycle ')
test=Path(work.name)
prefix=test/'install'; bins=test/'bin'; home=test/'private data'
archive=args.archive.resolve()
windows=os.name=='nt'
env=os.environ.copy(); env['LINGSHU_HOME']=str(home)
with socket.socket() as port_probe:
    port_probe.bind(('127.0.0.1',0)); server_port=port_probe.getsockname()[1]
installer=['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(root/'scripts/install.ps1')] if windows else ['sh',str(root/'scripts/install.sh')]
http_client=urllib.request.build_opener(urllib.request.ProxyHandler({}))
checks=[]
mirror=test/'releases'; assets=mirror/'latest/download'; assets.mkdir(parents=True)
shutil.copy2(archive,assets/archive.name)
shutil.copy2(Path(str(archive)+'.sha256'),assets/(archive.name+'.sha256'))
class QuietDownloads(http.server.SimpleHTTPRequestHandler):
    def log_message(self,*args):pass
downloads=http.server.ThreadingHTTPServer(('127.0.0.1',0),functools.partial(QuietDownloads,directory=str(mirror)))
threading.Thread(target=downloads.serve_forever,daemon=True).start()
def run(args,ok=True):
    result=subprocess.run([str(arg) for arg in args],env=env,cwd=test,capture_output=True,text=True,encoding='utf-8',timeout=150)
    if ok and result.returncode!=0: raise AssertionError(str(args)+'\n'+result.stdout+'\n'+result.stderr)
    return result

def request(url,body=None,token=None):
    headers={'Content-Type':'application/json'}
    if token:headers['Authorization']='Bearer '+token
    req=urllib.request.Request(url,data=None if body is None else json.dumps(body).encode(),headers=headers)
    with http_client.open(req,timeout=10) as response:return json.loads(response.read())

cli=prefix/'current/lingshu.exe' if windows else bins/'lingshu'
def wait_removed(path):
    for _ in range(150):
        if not path.exists():return
        time.sleep(.2)
    raise AssertionError('Uninstall did not remove '+str(path))
def active_release():return (prefix/'current').resolve()
try:
    env['LINGSHU_DOWNLOAD_BASE_URL']='http://127.0.0.1:'+str(downloads.server_port)
    run(installer+['--prefix',prefix,'--bin-dir',bins])
    env.pop('LINGSHU_DOWNLOAD_BASE_URL')
    checks.append('Online platform-specific archive and checksum download: PASS')
    assert cli.exists(); assert run([cli,'--version']).stdout.strip()=='lingshu 1.6.2'
    assert run([cli,'--home',prefix,'status'],ok=False).returncode != 0
    assert run([cli,'--home',Path.home(),'status'],ok=False).returncode != 0
    checks.append('Offline checksum-verified installation with paths containing spaces and safe data-directory guards: PASS')
    if not windows:
        commands=test/'authenticated download';commands.mkdir()
        gh=commands/'gh'
        gh.write_text('#!/usr/bin/env python3\nimport json,os,pathlib,shutil,sys\nargs=sys.argv[1:]\nif args==["auth","status"]:sys.exit(0)\nassert args[:3]==["release","download","v1.6.2"],args\nassert args[args.index("--repo")+1]=="LingeringDream/LingShu",args\nsource=pathlib.Path(os.environ["LINGSHU_TEST_ARCHIVE"])\ndestination=pathlib.Path(args[args.index("--dir")+1])\nshutil.copy2(source,destination/source.name)\nshutil.copy2(str(source)+".sha256",destination/(source.name+".sha256"))\npathlib.Path(os.environ["LINGSHU_TEST_GH_LOG"]).write_text(json.dumps(args))\n')
        gh.chmod(0o755)
        saved_path=env['PATH'];env['PATH']=str(commands)+os.pathsep+saved_path
        env['LINGSHU_TEST_ARCHIVE']=str(archive);env['LINGSHU_TEST_GH_LOG']=str(test/'gh-download.json')
        try:run(installer+['--version','1.6.2','--prefix',prefix,'--bin-dir',bins])
        finally:
            env['PATH']=saved_path;env.pop('LINGSHU_TEST_ARCHIVE');env.pop('LINGSHU_TEST_GH_LOG')
        assert (test/'gh-download.json').exists()
        checks.append('Authenticated GitHub release download with version normalization: PASS')
    result=run([cli,'status','--json'],ok=False); assert result.returncode==1 and not json.loads(result.stdout)['running']
    run([cli,'start','--port',str(server_port)])
    first=json.loads(run([cli,'status','--json']).stdout); assert first['ready']; config=json.loads((home/'config.json').read_text())
    assert (home/'postgresql/data/PG_VERSION').read_text().strip()=='17'
    checks.append('Fresh self-contained PostgreSQL initialization and detached service readiness: PASS')
    run([cli,'start']); assert json.loads(run([cli,'status','--json']).stdout)['pid']==first['pid']
    result=run([cli,'start','--port',str(server_port+1 if server_port<65535 else server_port-1)],ok=False); assert result.returncode!=0
    checks.append('Idempotent start and refusal to change running port: PASS')
    runtime=json.loads((home/'runtime.json').read_text())
    try:request('http://127.0.0.1:'+str(runtime['control_port'])+'/stop',{})
    except urllib.error.HTTPError as error:assert error.code==401
    else:raise AssertionError('unauthorized shutdown accepted')
    checks.append('Supervisor rejects unauthenticated shutdown: PASS')
    base=first['url']; session=request(base+'/api/v1/auth/local-session',{}); token=session['token']
    request(base+'/api/v1/memories',{'memory_type':'fact','content':'CLI 升级验证：PostgreSQL','importance':0.7},token)
    memories=request(base+'/api/v1/memories',token=token); assert any('CLI 升级验证' in m['content'] for m in memories)
    with http_client.open(base) as response:assert 'AI 共享记忆' in response.read().decode()
    assert run([cli,'open','--no-browser']).stdout.strip()==base
    checks.append('Real memory API and same-origin bundled web console: PASS')
    for tool,suffix in [('cursor','json'),('claude','json'),('codex','toml')]:
        path=test/(tool+'.'+suffix); original='{"personal":42,"mcpServers":{"other":{"command":"other"}}}' if suffix=='json' else '# personal\nmodel = "custom"\n[mcp_servers.other]\ncommand="other"\n'
        path.write_text(original)
        run([cli,'connect',tool,'--config',path]); assert 'other' in path.read_text()
        assert list(test.glob(path.name+'.lingshu-backup-*'))
        if suffix=='json':
            command=Path(json.loads(path.read_text())['mcpServers']['lingshu-memory']['command'])
            assert command.parent.name=='current'
            assert command.resolve()==(prefix/'current'/('lingshu.exe' if windows else 'lingshu')).resolve()
    checks.append('Cursor/Claude/Codex configuration preserves existing entries and backs up originals: PASS')
    run([cli,'stop']); assert not (home/'runtime.json').exists()
    assert not (home/'postgresql/data/postmaster.pid').exists()
    assert json.loads(run([cli,'status','--json'],ok=False).stdout)['running']==False
    checks.append('Graceful stop shuts down backend and embedded database: PASS')
    frames=[{'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'smoke','version':'1'}}},{'jsonrpc':'2.0','id':2,'method':'tools/list','params':{}},{'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'lingshu_stats','arguments':{}}}]
    frames.extend([
        {'jsonrpc':'2.0','id':4,'method':'tools/call','params':{'name':'lingshu_store','arguments':{'agent_id':'cursor','memory_layer':'fact','memory_scope':'global','context_scope':'cli-test','fact_key':'database.engine','fact_value':'PostgreSQL','content':'CLI shared memory uses PostgreSQL'}}},
        {'jsonrpc':'2.0','id':5,'method':'tools/call','params':{'name':'lingshu_recall','arguments':{'query':'CLI shared memory PostgreSQL','agent_id':'claude','context_scope':'cli-test','layer':'fact'}}},
    ])
    result=subprocess.run([str(cli),'mcp'],input='\n'.join(map(json.dumps,frames))+'\n',env=env,cwd=test,capture_output=True,text=True,encoding='utf-8',timeout=150)
    assert result.returncode==0, result.stderr
    replies=[json.loads(line) for line in result.stdout.splitlines()]; assert len(replies)==5,replies
    assert len(replies[1]['result']['tools'])==6 and 'result' in replies[2],replies
    assert all(not reply.get('result',{}).get('isError',False) for reply in replies),replies
    assert 'CLI shared memory uses PostgreSQL' in json.dumps(replies[4]),replies
    checks.append('Cold-start stdio MCP handshake, six tools, live stats and cross-agent write/recall; stdout only JSON-RPC: PASS')
    assert json.loads(run([cli,'status','--json']).stdout)['ready']
    old_release=active_release()
    run([cli,'update','--archive',archive])
    assert active_release()!=old_release
    second=json.loads(run([cli,'status','--json']).stdout); assert second['ready'] and second['pid']!=first['pid']
    assert json.loads((home/'config.json').read_text())['jwt_secret']==config['jwt_secret']
    session=request(base+'/api/v1/auth/local-session',{})
    assert any('CLI 升级验证' in m['content'] for m in request(base+'/api/v1/memories',token=session['token']))
    checks.append('Version-directory upgrade switches binaries, restarts service, preserves secrets and real memories: PASS')
    bad=test/'bad.tar.gz';bad.write_bytes(archive.read_bytes());Path(str(bad)+'.sha256').write_text('0'*64+'  bad.tar.gz\n')
    previous=active_release()
    result=run(installer+['--archive',bad,'--prefix',prefix,'--bin-dir',bins],ok=False)
    assert result.returncode!=0 and active_release()==previous
    assert json.loads(run([cli,'status','--json']).stdout)['ready']
    result=run([cli,'update','--archive',bad],ok=False)
    assert result.returncode!=0 and active_release()==previous
    assert json.loads(run([cli,'status','--json']).stdout)['ready']
    checks.append('Corrupt archive rejected before changing running installation: PASS')
    run([cli,'uninstall'])
    wait_removed(prefix)
    assert not prefix.exists() and not cli.exists() and (home/'config.json').exists() and (home/'postgresql/data/PG_VERSION').exists()
    checks.append('Uninstall removes programs, stops PostgreSQL and retains memory data: PASS')
    run(installer+['--archive',archive,'--prefix',prefix,'--bin-dir',bins])
    run([cli,'start'])
    run([cli,'uninstall','--purge'])
    wait_removed(prefix)
    assert not home.exists()
    checks.append('Explicit purge removes only the recognized CLI data directory: PASS')
    print('\n'.join(checks))
finally:
    if cli.exists():run([cli,'stop'],ok=False)
    if cli.exists():run(installer+['--prefix',prefix,'--uninstall'],ok=False)
    downloads.shutdown(); downloads.server_close()
    work.cleanup()
