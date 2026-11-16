import os, subprocess

url = 'postgres://vink266@127.0.0.1:55566/vink266_focused'
env = dict(os.environ, DATABASE_URL=url, VINK_MIGRATE_DATABASE_URL=url, VINK_PROFILE='local')
binary = 'target/debug/vink-app'
def run(*args):
    result = subprocess.run([binary, *args], env=env, text=True, capture_output=True, check=True)
    print(result.stdout, end='')
    print(result.stderr, end='')
    return result.stdout
run('migrate')
run('dev-seed')
ids = []
for version, (facet, label) in enumerate([
    ('form', 'Tản văn'), ('form', 'Thơ'), ('theme', 'Ký ức'),
    ('theme', 'Gia đình'), ('theme', 'Mùa'), ('theme', 'Đô thị'),
], 1):
    output = run('tag', 'create', str(version), facet, label)
    ids.append(next(line.split('\t')[1] for line in output.splitlines() if line.startswith('tag\t')))
run('tag', 'alias', '7', ids[2], 'Hồi ức')
run('tag', 'describe', '8', ids[2], 'Những điều đã qua còn ở lại trong người viết.')
