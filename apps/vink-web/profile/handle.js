// The enclosing Leptos session mounts a fresh host on every account change.
// All server text enters the DOM as text nodes. Only a confirmed read supplies quota.
const api = '/api/v1/me/handle';
const manage = '/toi/dia-chi';
const issue = {
  validation_failed:
    'Dùng 1–40 chữ thường a–z, số và dấu gạch nối giữa các nhóm. Một số tên được dành riêng.',
  handle_taken: 'Địa chỉ này đã có người dùng. Hãy chọn tên khác.',
  handle_alias_reserved: 'Đây là địa chỉ cũ của bạn. Hiện chưa thể dùng lại.',
  handle_quota_exhausted: 'Bạn đã dùng hết hai lượt đổi.',
};
function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
  node.append(...children);
  return node;
}
function button(label, action, secondary = false) {
  const btn = el(
    'button',
    { type: 'button', class: secondary ? 'vk-handle-text' : 'vk-handle-btn' },
    label,
  );
  btn.onclick = action;
  return btn;
}
const heading = (text) => el('p', { class: 'vk-handle-kicker' }, text);
const valid = (text) => text.length <= 40 && /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(text);
const actions = (...controls) => el('div', { class: 'vk-handle-actions' }, ...controls);
const address = (name) => '/@' + name;
async function request(path, command) {
  const response = await fetch(path, {
    method: command ? 'POST' : 'GET',
    cache: 'no-store',
    credentials: 'same-origin',
    signal: AbortSignal.timeout(30000),
    headers: command ? { 'content-type': 'application/json' } : {},
    body: command ? JSON.stringify(command) : undefined,
  });
  const data = await response.json();
  if (!response.ok)
    throw Object.assign(new Error('handle request'), {
      status: response.status,
      code: data.error?.code,
    });
  return data;
}

export function mountHandle(host, principal, entry) {
  const storageKey = 'vink.handle.pending.' + principal;
  let state,
    draft,
    pending,
    busy = false,
    editing = false,
    notice = '';
  const readPending = () => {
    try {
      const c = JSON.parse(sessionStorage.getItem(storageKey));
      return c?.principal_id === principal &&
        typeof c.handle === 'string' &&
        typeof c.command_id === 'string'
        ? c
        : null;
    } catch {
      return null;
    }
  };
  pending = readPending();
  function remember(cmd) {
    pending = cmd;
    try {
      if (cmd) sessionStorage.setItem(storageKey, JSON.stringify(cmd));
      else sessionStorage.removeItem(storageKey);
    } catch {
      /* The current page can still reconcile without browser storage. */
    }
  }
  const messages = el('p', { role: 'status', 'aria-live': 'polite' });
  async function refresh() {
    const own = await request(api);
    if (!host.isConnected || own.principal_id !== principal) throw new Error('scope changed');
    state = own;
  }
  function unavailable() {
    if (!host.isConnected) return;
    host.replaceChildren(
      el('p', { role: 'status' }, 'Chưa tải được địa chỉ tác giả.'),
      button('Thử lại', load),
    );
  }
  async function load() {
    try {
      await refresh();
      draft = pending?.handle ?? state.handle;
      editing = !state.setup_complete || !!pending;
      render();
      if (entry && location.hash === '#dia-chi-tac-gia') host.querySelector('a')?.focus();
    } catch {
      unavailable();
    }
  }
  function preview() {
    const initials = state.pen_name
      .split(/\s+/u)
      .slice(0, 2)
      .map((w) => Array.from(w)[0])
      .join('')
      .toUpperCase();
    const plate = el(
      'article',
      { class: 'vk-author-plate' },
      el('div', { class: 'vk-author-art', 'aria-hidden': 'true' }),
      el(
        'div',
        { class: 'vk-author-info' },
        el('span', { class: 'vk-author-seal', 'aria-hidden': 'true' }, initials),
        el('h2', {}, state.pen_name),
        el('p', { id: 'preview-handle' }, '@' + state.handle),
      ),
    );
    if (!state.has_public_works)
      plate.append(el('p', { class: 'vk-author-foot' }, 'Chưa có tác phẩm công khai'));
    return el(
      'aside',
      { 'aria-label': 'Xem trước hồ sơ công khai' },
      heading('Xem trước hồ sơ'),
      plate,
      el('p', { class: 'vk-muted' }, 'Bút danh và địa chỉ như người đọc sẽ thấy.'),
    );
  }
  function summary() {
    const box = el(
      'section',
      {},
      heading('Địa chỉ hiện tại'),
      el('h2', { tabindex: '-1', id: 'handle-saved' }, '@' + state.handle),
      el(
        'p',
        {},
        state.remaining_changes
          ? 'Bạn còn ' + state.remaining_changes + ' lượt đổi. Mỗi lần đổi đều giữ lại liên kết cũ.'
          : 'Bạn đã dùng hết hai lượt đổi. Địa chỉ này tiếp tục dẫn đến hồ sơ của bạn.',
      ),
    );
    const row = actions(
      el(
        'a',
        { class: 'vk-handle-btn', href: address(state.handle), rel: 'external' },
        'Xem trang tác giả',
      ),
    );
    if (state.remaining_changes)
      row.append(
        button(
          'Đổi địa chỉ',
          () => {
            editing = true;
            draft = state.handle;
            notice = '';
            render();
            fieldFocus();
          },
          true,
        ),
      );
    box.append(row);
    if (state.aliases.length) {
      const history = el(
        'details',
        {},
        el('summary', {}, 'Liên kết cũ (' + state.aliases.length + ')'),
      );
      for (const alias of state.aliases)
        history.append(
          el('p', {}, el('a', { href: address(alias), rel: 'external' }, address(alias))),
        );
      box.append(history);
    }
    return box;
  }
  const fieldFocus = () => host.querySelector('#author-handle')?.focus();
  function form() {
    const setup = !state.setup_complete;
    const box = el(
      'section',
      {},
      heading(pending ? 'Đang kiểm tra' : setup ? 'Thiết lập lần đầu' : 'Đổi địa chỉ'),
      el('h2', {}, setup ? 'Chọn địa chỉ của bạn' : 'Chọn địa chỉ mới'),
      el(
        'p',
        {},
        setup
          ? 'Giữ tên gợi ý hoặc nhập tên riêng. Lần thiết lập này không trừ lượt đổi.'
          : 'Nhập địa chỉ mới rồi kiểm tra lại trước khi xác nhận.',
      ),
    );
    if (!setup)
      box.append(
        el(
          'p',
          { class: 'vk-handle-current' },
          'Hiện tại @' + state.handle + ' · Còn ' + state.remaining_changes + ' lượt đổi',
        ),
      );
    const input = el('input', {
      id: 'author-handle',
      name: 'handle',
      autocomplete: 'off',
      autocapitalize: 'none',
      spellcheck: 'false',
      'aria-describedby': 'handle-help handle-error',
    });
    input.value = draft;
    input.readOnly = !!pending;
    const error = el('p', { id: 'handle-error', 'aria-live': 'polite', class: 'vk-handle-error' });
    const url = el('output', {}, address(valid(draft) ? draft : state.handle));
    const change = () => {
      draft = input.value;
      error.textContent = '';
      input.removeAttribute('aria-invalid');
      const shown = valid(draft) ? draft : state.handle;
      url.textContent = address(shown);
      host.querySelector('#preview-handle').textContent = '@' + shown;
    };
    input.oninput = change;
    const field = el(
      'form',
      { novalidate: '' },
      el('label', { for: input.id }, 'Tên trong liên kết'),
      el('div', { class: 'vk-handle-input' }, el('span', { 'aria-hidden': 'true' }, '@'), input),
      el(
        'p',
        { id: 'handle-help' },
        'Dùng 1–40 chữ thường a–z, số 0–9 và dấu gạch nối giữa các nhóm.',
      ),
      error,
      el('p', {}, 'Đường dẫn dự kiến ', url),
    );
    const submit = el(
      'button',
      { type: 'submit', class: 'vk-handle-btn' },
      setup ? 'Dùng địa chỉ này' : 'Xem thay đổi',
    );
    let composing = false;
    input.oncompositionstart = () => {
      composing = true;
    };
    input.oncompositionend = () => {
      composing = false;
      change();
    };
    field.onsubmit = (e) => {
      e.preventDefault();
      if (busy || pending || composing) return;
      if (!valid(input.value)) return fieldError(issue.validation_failed);
      if (!setup && draft === state.handle) return fieldError('Đây là địa chỉ hiện tại của bạn.');
      error.textContent = '';
      input.removeAttribute('aria-invalid');
      if (setup) sendNew();
      else confirmRename(submit);
    };
    if (pending) field.append(actions(button('Kiểm tra kết quả', reconcile)));
    else
      field.append(
        actions(
          submit,
          setup
            ? el('a', { class: 'vk-handle-text', href: '/toi#dia-chi-tac-gia' }, 'Để sau')
            : button(
                'Hủy',
                () => {
                  editing = false;
                  notice = '';
                  render();
                  host.querySelector('#handle-saved').focus();
                },
                true,
              ),
        ),
      );
    box.append(
      field,
      el(
        'p',
        { class: 'vk-handle-note' },
        setup
          ? 'Hai lượt đổi tên sau lần thiết lập đầu tiên.'
          : state.remaining_changes === 1
            ? 'Đây là lượt đổi cuối cùng của tài khoản.'
            : 'Sau lần đổi này, bạn sẽ còn một lượt đổi.',
      ),
      el(
        'p',
        { class: 'vk-handle-note' },
        'Những liên kết đã chia sẻ vẫn dẫn đến trang tác giả của bạn.',
      ),
    );
    return box;
  }
  function render() {
    if (!host.isConnected) return;
    if (entry) {
      host.replaceChildren(
        el('p', {}, 'Địa chỉ tác giả · @' + state.handle),
        el(
          'a',
          { href: manage, class: 'vk-handle-btn' },
          pending
            ? 'Kiểm tra địa chỉ'
            : state.setup_complete
              ? 'Quản lý địa chỉ'
              : 'Thiết lập địa chỉ',
        ),
      );
      return;
    }
    messages.textContent =
      notice ||
      (pending ? 'Chưa nhận được xác nhận. Hãy kiểm tra kết quả trước khi đổi tiếp.' : '');
    host.replaceChildren(
      messages,
      el('div', { class: 'vk-handle-columns' }, editing ? form() : summary(), preview()),
    );
    const previewHandle = host.querySelector('#preview-handle');
    if (editing && valid(draft)) previewHandle.textContent = '@' + draft;
  }
  function fieldError(message) {
    const error = host.querySelector('#handle-error');
    if (!error) return;
    error.textContent = message;
    host.querySelector('#author-handle').setAttribute('aria-invalid', 'true');
    fieldFocus();
  }
  function confirmRename(trigger) {
    const title = el('h2', { id: 'handle-confirm-title', tabindex: '-1' }, 'Đổi địa chỉ tác giả?');
    const dialog = el(
      'dialog',
      { 'aria-labelledby': title.id, class: 'vk-handle-dialog' },
      title,
      el(
        'dl',
        { class: 'vk-handle-compare' },
        el('div', {}, el('dt', {}, 'Địa chỉ hiện tại'), el('dd', {}, '@' + state.handle)),
        el('div', {}, el('dt', {}, 'Địa chỉ mới'), el('dd', {}, '@' + draft)),
      ),
      el('p', {}, 'Mọi liên kết cũ sẽ mở thẳng trang tác giả ở địa chỉ mới.'),
      el(
        'p',
        {},
        state.remaining_changes === 1
          ? 'Đây là lượt cuối. Sau lần này bạn sẽ hết lượt đổi.'
          : 'Sau lần này còn một lượt đổi.',
      ),
    );
    const close = button('×', () => dialog.close(), true);
    close.classList.add('vk-handle-close');
    close.setAttribute('aria-label', 'Đóng xác nhận');
    dialog.append(
      close,
      actions(
        button('Xác nhận đổi', () => dialog.close('confirm')),
        button('Quay lại sửa', () => dialog.close(), true),
      ),
    );
    dialog.onclose = () => {
      dialog.remove();
      if (dialog.returnValue === 'confirm') sendNew();
      else trigger.focus();
    };
    host.append(dialog);
    dialog.showModal();
    title.focus();
  }
  function sendNew() {
    if (busy || pending) return;
    remember({
      principal_id: principal,
      command_id: crypto.randomUUID(),
      expected_version: state.version,
      intent: state.setup_complete ? 'rename' : 'setup',
      handle: draft,
    });
    void submitPending();
  }
  async function accepted(receipt) {
    if (
      receipt.command_id !== pending.command_id ||
      receipt.handle !== pending.handle ||
      !Number.isSafeInteger(receipt.accepted_version)
    )
      throw new Error('unconfirmed command');
    await refresh();
    if (state.version < receipt.accepted_version) throw new Error('stale own read');
    remember(null);
    draft = state.handle;
    editing = false;
    notice = 'Đã lưu địa chỉ tác giả.';
    render();
    host.querySelector('#handle-saved')?.focus();
  }
  async function submitPending() {
    if (busy || !pending) return;
    busy = true;
    notice = 'Đang lưu địa chỉ…';
    render();
    try {
      await accepted(await request(api, pending));
    } catch (err) {
      if (!host.isConnected) return;
      if (err.status >= 400 && err.status < 500) {
        remember(null);
        try {
          await refresh();
        } catch {
          unavailable();
          return;
        }
        editing = !state.setup_complete || state.remaining_changes > 0;
        notice =
          err.code === 'version_conflict'
            ? 'Địa chỉ đã thay đổi ở phiên khác. Hãy kiểm tra lại trước khi lưu.'
            : '';
        render();
        fieldError(issue[err.code] || notice || 'Chưa thể lưu địa chỉ này.');
      } else {
        notice = 'Chưa nhận được xác nhận. Địa chỉ và số lượt sẽ cập nhật khi có kết quả.';
        render();
      }
    } finally {
      busy = false;
    }
  }
  async function reconcile() {
    if (busy || !pending) return;
    busy = true;
    messages.textContent = 'Đang kiểm tra kết quả…';
    try {
      await accepted(await request(api + '/receipts/' + pending.command_id));
    } catch (err) {
      if (!host.isConnected) return;
      notice = 'Chưa kiểm tra được kết quả. Bạn có thể thử lại.';
      render();
      if (err.status === 404) {
        messages.textContent = 'Chưa tìm thấy lệnh đã lưu. Bạn có thể gửi lại đúng yêu cầu này.';
        host.querySelector('form').append(button('Gửi lại yêu cầu', submitPending, true));
      }
    } finally {
      busy = false;
    }
  }
  host.replaceChildren(el('p', { role: 'status' }, 'Đang tải địa chỉ tác giả…'));
  void load();
}
