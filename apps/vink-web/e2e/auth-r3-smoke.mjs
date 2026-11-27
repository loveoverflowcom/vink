// Shared Auth composition on the actual Product hosts. The caller supplies the route,
// theme and text size, and decides which matched scenes to capture.
const modes = {
  login: { form: '#password-login', inputs: ['#password-login-username', '#password-login-password'],
    icons: ['person-outline', 'lock'], primary: ['#password-login-submit', 'Đăng nhập', 'lock'],
    method: ['#use-email-code', 'Dùng mã email', 'mail'],
    account: ['#use-registration', 'Tạo tài khoản', 'Chưa có tài khoản?', 'person-add'] },
  registration: { form: '#registration-email', inputs: ['#registration-address'],
    icons: [null], primary: ['#registration-send', 'Gửi mã xác minh', 'mail'],
    account: ['#use-password', 'Đăng nhập', 'Đã có tài khoản?', 'lock'] },
  email: { form: '#sign-in-email', inputs: ['#sign-in-address'], icons: [null],
    primary: ['#sign-in-send', 'Gửi mã đăng nhập', 'mail'], method: ['#use-password', 'Dùng mật khẩu', 'lock'] },
};

function inspectAuth(mode) {
  const form = document.querySelector(mode.form);
  const entry = form.closest('.vk-auth-entry');
  const dialog = entry.closest('dialog');
  const find = (selector) => entry.querySelector(selector);
  const inputs = mode.inputs.map(find);
  const fields = inputs.map(input => input.closest('.vk-ui-field__control'));
  const labels = inputs.map(input => input.labels[0]);
  const buttons = [mode.primary, mode.method].filter(Boolean).map(([selector]) => find(selector));
  const account = mode.account && find(mode.account[0]);
  const accountRow = find('.vk-auth-account-switch');
  const logo = (dialog ?? entry).querySelector('.vk-brand__lockup');
  const heading = (dialog ?? entry).querySelector('h2');
  const lead = dialog?.querySelector('#reader-auth-lead');
  const bounds = entry.getBoundingClientRect();
  const palette = document.createElement('canvas').getContext('2d');
  const luminance = (color) => {
    palette.fillStyle = color;
    const rgb = palette.fillStyle.slice(1).match(/../g).map(value => parseInt(value, 16) / 255);
    return rgb.map(c => c <= .04045 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4)
      .reduce((sum, c, i) => sum + c * [.2126, .7152, .0722][i], 0);
  };
  const contrast = (a, b) => {
    const [low, high] = [luminance(a), luminance(b)].sort((x, y) => x - y);
    return (high + .05) / (low + .05);
  };
  const background = (element) => {
    for (let node = element; node; node = node.parentElement) {
      const fill = getComputedStyle(node).backgroundColor;
      if (fill !== 'rgba(0, 0, 0, 0)' && fill !== 'transparent') return fill;
    }
    return null;
  };
  const icon = (el) => el.querySelector('svg[data-icon][aria-hidden="true"]');
  const box = (el) => el.getBoundingClientRect();
  const leading = (el) => {
    const glyph = icon(el);
    return glyph && box(glyph).right <= box(el.querySelector('.vk-ui-button__label')).left
      ? glyph.dataset.icon : null;
  };
  const centered = (el) => Math.abs(box(el).left + box(el).width / 2 - (bounds.left + bounds.width / 2)) <= 1;
  const sameColumn = (el) => Math.abs(box(el).left - bounds.left) <= 1 && Math.abs(box(el).width - bounds.width) <= 1;
  const order = [...fields, ...buttons, ...(accountRow ? [accountRow] : [])];
  const gap = (first, second) => box(second).top - box(first).bottom;
  const unit = parseFloat(getComputedStyle(document.documentElement).fontSize);
  const host = background(entry);
  const frame = dialog?.getBoundingClientRect();
  return {
    host,
    order: order.every((el, i) => sameColumn(el) && (!i || (
      order[i - 1].compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING &&
      box(el).top >= box(order[i - 1]).bottom - 1
    ))),
    noBackOrDivider: !find('.vk-auth-divider, .vk-ui-back-link, [data-icon="arrow-back"]') &&
      !entry.innerText.includes('Quay lại đăng nhập') && ![...entry.querySelectorAll('p')].some(p => p.textContent.trim() === 'Hoặc'),
    noRegistrationEmailShortcut: mode.form !== '#registration-email' || !find('#use-email-code'),
    noDeletion: !(dialog ?? document.querySelector('#account-session'))
      .querySelector('#open-account-deletion, #account-deletion-settings-link, #delete-account-form'),
    fits: entry.scrollWidth <= entry.clientWidth + 1 && document.documentElement.scrollWidth <= innerWidth + 1,
    actionGap: !mode.method || Math.abs(gap(buttons[0], buttons[1]) - unit / 2) <= 1,
    account: account && accountRow ? {
      text: account.textContent.trim(), prompt: accountRow.firstElementChild.textContent.trim(),
      leading: leading(account),
      variant: account.classList.contains('vk-ui-button--text'),
      type: account.type, height: box(account).height, width: box(account).width,
      contrast: contrast(getComputedStyle(account).color, host),
      promptContrast: contrast(getComputedStyle(accountRow.firstElementChild).color, host),
      fits: accountRow.scrollWidth <= accountRow.clientWidth + 1 && account.scrollWidth <= account.clientWidth + 1,
      spacing: Math.abs(gap(buttons.at(-1), accountRow) - unit) <= 1,
    } : null,
    heading: centered(logo) && centered(heading) && getComputedStyle(heading).textAlign === 'center' &&
      (!lead || centered(lead) && getComputedStyle(lead).textAlign === 'center'),
    modalGeometry: !dialog || (
      frame.width <= 30 * parseFloat(getComputedStyle(document.documentElement).fontSize) + 1 &&
      dialog.scrollWidth <= dialog.clientWidth + 1 &&
      (innerWidth < 760
        ? Math.abs(frame.bottom - innerHeight) <= 1 && frame.left === 0 && frame.right === innerWidth
        : Math.abs(frame.left + frame.right - innerWidth) <= 2 && Math.abs(frame.top + frame.bottom - innerHeight) <= 2)
    ),
    fields: fields.map((el, i) => {
      const style = getComputedStyle(el);
      const label = labels[i];
      const glyph = icon(el);
      const fill = style.backgroundColor;
      const edge = style.outlineColor;
      return { height: box(el).height, fill, edge, outline: style.outlineWidth, border: style.borderTopWidth,
        radius: style.borderTopLeftRadius, outlineStyle: style.outlineStyle,
        fillContrast: contrast(fill, host), edgeContrast: contrast(edge, host),
        textContrast: contrast(getComputedStyle(label).color, fill),
        inputContrast: contrast(getComputedStyle(inputs[i]).color, fill),
        revealContrast: el.querySelector('.vk-ui-field__reveal button')
          ? contrast(getComputedStyle(el.querySelector('.vk-ui-field__reveal button')).color, fill) : null,
        focused: inputs[i] === document.activeElement,
        icon: glyph?.dataset.icon, iconContrast: glyph ? contrast(getComputedStyle(glyph).color, fill) : null,
        labelFits: (!glyph || box(label).left >= box(glyph).right) && label.scrollWidth <= label.clientWidth + 1,
        native: label.htmlFor === inputs[i].id && inputs[i].getAttribute('aria-required') === 'true',
      };
    }),
    buttons: buttons.map(el => {
      const style = getComputedStyle(el);
      const glyph = icon(el);
      return { text: el.textContent.trim(), type: el.type, height: box(el).height, width: box(el).width, border: style.borderTopWidth,
        tonal: el.classList.contains('vk-ui-button--tonal'),
        filled: el.classList.contains('vk-ui-button--filled'),
        fill: style.backgroundColor, contrast: contrast(style.color, style.backgroundColor),
        icon: leading(el), iconContrast: glyph ? contrast(getComputedStyle(glyph).color, style.backgroundColor) : null,
        clip: el.scrollWidth > el.clientWidth + 1,
      };
    }),
  };
}

export async function checkAuthR31(page, check, scene, mode = 'login') {
  const spec = modes[mode];
  const result = await page.eval(
    '(' + inspectAuth.toString() + ')(' + JSON.stringify(spec) + ')',
  );
  const { account, buttons } = result;
  check(result.order && result.noBackOrDivider && result.noRegistrationEmailShortcut && result.noDeletion &&
    result.actionGap && result.fits && result.heading && result.modalGeometry &&
    result.fields.every((f, i) => f.height >= 56 && f.radius === '16px' &&
      (f.focused ? f.outlineStyle === 'solid' && f.outline === '2px' : f.outlineStyle === 'none') && f.border === '0px' &&
      f.fillContrast >= 3 && (!f.focused || f.edgeContrast >= 3) &&
      f.textContrast >= 4.5 && f.inputContrast >= 4.5 &&
      (f.revealContrast === null || f.revealContrast >= 3) &&
      (f.icon ?? null) === spec.icons[i] && (f.iconContrast === null || f.iconContrast >= 3) && f.native && f.labelFits) &&
    buttons.every(b => b.height >= 48 && b.border === '0px' && b.contrast >= 4.5 && !b.clip) &&
    buttons[0].filled && buttons[0].text === spec.primary[1] &&
    buttons[0].icon === spec.primary[2] && buttons[0].iconContrast >= 3 && buttons[0].type === 'submit' &&
    (!spec.method || buttons[1].tonal && buttons[1].text === spec.method[1] &&
      buttons[1].icon === spec.method[2] && buttons[1].iconContrast >= 3 && buttons[1].type === 'button') &&
    (spec.account ? account?.variant && account.type === 'button' && account.prompt === spec.account[2] &&
      account.text === spec.account[1] && account.leading === spec.account[3] && account.height >= 48 && account.width < buttons[0].width &&
      account.contrast >= 4.5 && account.promptContrast >= 4.5 && account.fits && account.spacing : !account),
    '#559 ' + scene + ': ' + mode + ' geometry, hierarchy and contrast ' + JSON.stringify(result));
  return result;
}

export async function checkModeSwitches(page, check, scene) {
  const locationBefore = await page.eval('JSON.stringify([location.href, history.length, performance.timeOrigin])');
  const posts = [];
  const listener = (event) => {
    if (event.sessionId === page.sessionId && event.method === 'Network.requestWillBeSent') {
      const { method, url } = event.params.request;
      if (method === 'POST' && new URL(url).pathname.startsWith('/api/v1/auth/')) posts.push(new URL(url).pathname);
    }
  };
  page.cdp.listeners.push(listener);
  try {
    for (const [control, firstField] of [
      ['use-registration', 'registration-address'],
      ['use-password', 'password-login-username'],
      ['use-email-code', 'sign-in-address'],
      ['use-password', 'password-login-username'],
    ]) {
      await page.eval(`document.getElementById('${control}').focus()`);
      await page.key('Enter');
      await page.waitFor(`document.activeElement?.id === '${firstField}'`, `#559 ${scene}: keyboard switch focuses ${firstField}`);
    }
    const locationAfter = await page.eval('JSON.stringify([location.href, history.length, performance.timeOrigin])');
    check(locationBefore === locationAfter && posts.length === 0,
      `#559 ${scene}: keyboard mode changes stay in the same document and history entry and send no Auth requests`);
  } finally {
    page.cdp.listeners = page.cdp.listeners.filter(item => item !== listener);
  }
}

const authSnapshot = function (selector) {
  const button = document.querySelector(selector);
  const label = button.querySelector('.vk-ui-button__label');
  const glyph = button.querySelector('svg[data-icon][aria-hidden=true]');
  const spinner = button.querySelector('.vk-ui-arc');
  const box = (node) => {
    const { x, y, width, height } = node.getBoundingClientRect();
    return [x + scrollX, y + scrollY, width, height];
  };
  return {
    button: box(button), label: box(label), text: label.textContent.trim(),
    focus: document.activeElement.id, icon: glyph?.dataset.icon,
    spinnerCount: button.querySelectorAll('.vk-ui-arc').length,
    leadingSpinner: !!spinner?.closest('[data-slot=leading]') &&
      spinner.getBoundingClientRect().right <= label.getBoundingClientRect().left,
    busy: button.getAttribute('aria-busy'),
    disabled: button.getAttribute('aria-disabled'),
    status: document.getElementById(button.getAttribute('aria-describedby'))?.textContent.trim(),
  };
};

export async function snapshotAuthAction(page, selector) {
  return page.eval('(' + authSnapshot.toString() + ')(' + JSON.stringify(selector) + ')');
}

export async function checkAuthPending(page, check, selector, idle, icon, label, focus) {
  const pending = await snapshotAuthAction(page, selector);
  const stable = (a, b) => a.every((value, i) => Math.abs(value - b[i]) <= 1);
  check(idle.icon === icon && idle.spinnerCount === 0 && idle.text === label &&
    pending.icon === undefined && pending.spinnerCount === 1 && pending.leadingSpinner &&
    pending.text === label && pending.focus === focus && pending.busy === 'true' &&
    pending.disabled === 'true' && pending.status &&
    stable(idle.button, pending.button) && stable(idle.label, pending.label),
    '#625 ' + selector + ' holds geometry and focus with one leading pending indicator ' + JSON.stringify({ idle, pending }));
}
