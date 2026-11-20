# Read only: inspect source manifests and proxy policy, without Compose, secrets, DNS or delivery.
require 'yaml'
require 'pathname'

root = Pathname.new(__dir__).parent.parent
def check(condition, message)
  abort(message) unless condition
end

def manifest(file)
  YAML.safe_load_file(file)
end

mail_mount = '/run/vink-resend:ro'
proxy = 'http://vink-resend-egress:3128'
realms = { 'production' => 'prod', 'test' => 'test' }
realms.each do |realm, stub|
  prefix = 'VINK_' + stub.upcase
  base = manifest(root / "infra/#{realm}/compose.yaml")
  overlay = manifest(root / "infra/#{realm}/auth-resend.compose.yaml")
  base_services = base.fetch('services')
  services = overlay.fetch('services')
  check(services.keys.sort == (stub == 'test' ? %w[app identity postgres] : %w[app identity]), "#{realm}: auth overlay changes unrelated services")
  check(base_services['app']['environment']['VINK_AUTH_SIGN_IN'] == 'disabled', "#{realm}: base gate open")
  check(base_services['worker']['environment']['VINK_AUTH_SIGN_IN'] == 'disabled', "#{realm}: worker gate open")
  check(base_services['identity']['volumes'].none? { |v| v.end_with?(mail_mount) }, "#{realm}: key in base")
  app = services.fetch('app')
  identity = services.fetch('identity')
  check(app['environment']['VINK_AUTH_SIGN_IN'] == "\${#{prefix}_AUTH_SIGN_IN:-disabled}", "#{realm}: gate not isolated/fail closed")
  check(app.dig('depends_on', 'identity', 'condition') == 'service_healthy', "#{realm}: startup skips Identity readiness")
  check(!identity.key?('ports') && !base_services['identity'].key?('ports'), "#{realm}: Identity publishes a port")
  check(!base_services['postgres'].key?('ports'), "#{realm}: DB publishes a port")
  private_db = stub == 'test' ? 'auth-test-db' : 'default'
  check(identity['networks'].keys.sort == ['auth-mail', private_db], "#{realm}: Identity must not join an uplink")
  check(identity.dig('networks', 'auth-mail', 'ipv4_address').start_with?("\${#{prefix}_AUTH_MAIL_IDENTITY_IP:?"),
        "#{realm}: Identity has no required realm-specific IP for the proxy ACL")
  check(identity['volumes'].size == 1 && identity['volumes'][0].start_with?("\${#{prefix}_RESEND_SECRETS_DIR:?") &&
        identity['volumes'][0].end_with?(mail_mount), "#{realm}: Identity secret not an isolated read-only bind")
  env = identity.fetch('environment')
  check(env['VINK_IDENTITY_AUTHORITY'] == 'native' && env['VINK_IDENTITY_MAIL_TRANSPORT'] == 'resend-api' &&
        env['VINK_IDENTITY_RESEND_API_KEY_FILE'] == '/run/vink-resend/api-key' && env['VINK_IDENTITY_RESEND_PROXY'] == proxy &&
        env['VINK_IDENTITY_RESEND_FROM'].start_with?("\${#{prefix}_RESEND_FROM:?"),
        "#{realm}: missing required authority/mail prerequisites")
  check(overlay.dig('networks', 'auth-mail') == { 'name' => "vink-#{stub}-auth-mail", 'external' => true },
        "#{realm}: mail isolation is not realm-specific")
  if stub == 'test'
    check(!overlay['networks'].key?('default') && overlay.dig('networks', private_db) == { 'internal' => true } &&
          !app.key?('networks') && !app.key?('ports') &&
          services['postgres'] == { 'networks' => { 'default' => {}, private_db => { 'aliases' => ['postgres'] } } } &&
          !app['environment'].key?('VINK_PUBLIC_ORIGIN') &&
          base_services['app']['environment']['VINK_PUBLIC_ORIGIN'] == '${VINK_PUBLIC_ORIGIN:?}' &&
          base_services['app']['ports'].all? { |p| p.start_with?('127.0.0.1:') }, 'test: default bridge/loopback or private DB isolation changed')
  end
end

policy = (root / 'infra/auth-mail/squid.conf.example').read
ordered_rules = [
  'acl resend_api dstdomain -n api.resend.com',
  'acl resend_https port 443',
  'http_access deny !CONNECT',
  'http_access deny !identity_peer',
  'http_access deny !resend_api',
  'http_access deny !resend_https',
  'http_access deny to_localhost',
  'http_access deny to_linklocal',
  'http_access deny private_dst',
  'http_access allow identity_peer resend_api resend_https',
  'http_access deny all'
]
actual = policy.lines.map(&:strip).select { |line| line.start_with?('http_access ', 'acl resend_') }
check(actual == ordered_rules, 'proxy policy no longer permits only the exact HTTPS CONNECT allowlist')
puts 'Auth overlays: activation defaults, environment separation, secret mounts, networks and proxy policy OK (static only)'
