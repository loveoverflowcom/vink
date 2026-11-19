// #328: real Chrome, App, email-code Identity and scratch PostgreSQL. The runner seeds a
// public root and a closed publication; no application responses are fabricated here.
import { account, fill, HYDRATED, mailedCode, q } from './auth-smoke.mjs';
import { captureRequest, holdAnswer } from './auth-browser-network.mjs';

const WORK = '33333333-3333-4333-8333-333333333333';
const ROOT = '3ac70000-0000-4000-8000-000000000002';
const CLOSED = '3ac70000-0000-4000-8000-000000000003';
const HERE = `/tac-pham/${WORK}`;
const DISCUSSION = `/api/v1/works/${WORK}/discussion`;
const DIALOG = '#reader-auth';
const ROOT_EDITOR = '#root-composer';
const REPLY_EDITOR = `#reply-composer-${ROOT}`;
const ROOT_TEXT = 'Nháp nhận xét: Mưa vừa tạnh bên hiên.\nCó những chiều chỉ muốn ngồi yên.';
const REPLY_TEXT = 'Phản hồi ở đúng nhánh: chữ ữ, ợ và dòng sau.\nĐừng gửi trước khi tôi bấm.';
const active = (id) => `document.activeElement?.id === ${JSON.stringify(id)}`;
const standing = (state) => `${q(DIALOG)}?.dataset.standing === ${JSON.stringify(state)}`;
const isOpen = `${q(DIALOG)}?.open === true`;

async function snapshot(page, editor, key) {
  return page.eval(`(() => {
    const e = ${q(editor)};
    e.setSelectionRange(3, 12, 'backward');
    window[${JSON.stringify(key)}] = { e, reader: ${q('#reader-paper')}, y: scrollY, top: e.getBoundingClientRect().top };
    return { y: scrollY, maxY: document.scrollingElement.scrollHeight - innerHeight };
  })()`);
}

function preserved(editor, key, text, includeScroll = true) {
  return `(() => {
    const e = ${q(editor)}, before = window[${JSON.stringify(key)}];
    return e === before.e && ${q('#reader-paper')} === before.reader && e.value === ${JSON.stringify(text)} &&
      e.selectionStart === 3 && e.selectionEnd === 12 && e.selectionDirection === 'backward' &&
      ${includeScroll ? 'Math.abs(scrollY - before.y) <= 2' : 'true'};
  })()`;
}

export async function readerAuthClosed(page, { args, check }) {
  await page.goto(args.base + HERE);
  await page.waitFor(`${HYDRATED} && ${standing('anonymous')} && ${q(ROOT_EDITOR)}`, 'guest on disabled sign-in deployment');
  await page.typeAtEnd(ROOT_EDITOR, ROOT_TEXT);
  check(await page.eval(`
    !${q('meta[name="vink-sign-in"]')} && !${q(ROOT_EDITOR+'-auth')} && !${isOpen} &&
    !${q('#password-login')} && ${q(ROOT_EDITOR)}.value === ${JSON.stringify(ROOT_TEXT)} &&
    ${q(ROOT_EDITOR+'-send')}.getAttribute('aria-disabled') === 'true' &&
    ${q('#root-composer-send-note')}.textContent.includes('Vink chưa mở đăng nhập')
  `), '#328 actual disabled App deployment exposes no Reader Auth action; drafting still works');
}

export async function contextualReaderAuth(owner, { args, check, openPage, seen }) {
  console.log('Reader Auth R2: local sheet/dialog with owned draft, real Identity and explicit send.');
  const { browserContextId } = await owner.cdp.send('Target.createBrowserContext');
  try {
    const page = await openPage(browserContextId);
    const comments = captureRequest(page, 'POST', DISCUSSION + '/comments');
    const confirmations = captureRequest(page, 'POST', '/api/v1/auth/sign-in/complete');
    await page.goto(args.base + '/tac-pham/' + CLOSED);
    await page.waitFor(`${HYDRATED} && ${standing('anonymous')} && ${q(ROOT_EDITOR)}`, 'closed Reader, guest confirmed');
    const closed = await page.eval(`fetch('/api/v1/works/${CLOSED}/discussion/public').then(r => r.json())`);
    check(closed.settings.comments_open === false && await page.eval(`!${q(ROOT_EDITOR+'-auth')} && ${q(ROOT_EDITOR+'-send')}.getAttribute('aria-disabled') === 'true'`),
      '#328 no contextual entry on a real closed discussion, composer still editable');

    await page.goto(args.base + '/');
    await page.waitFor(`${HYDRATED} && ${q('a[href="'+HERE+'"]')}`, 'discovery link mounted');
    await page.click('a[href="'+HERE+'"]');
    await page.waitFor(`location.pathname === '${HERE}' && ${q(ROOT_EDITOR+'-auth')}`, 'guest Reader SPA route');
    await page.viewport(1280, 844);
    await page.send('Page.bringToFront');
    await page.typeAtEnd(ROOT_EDITOR, ROOT_TEXT);
    await page.eval(`${q(ROOT_EDITOR+'-auth')}.focus()`);
    await snapshot(page, ROOT_EDITOR, 'readerAuthRoot');
    await page.key('Enter');
    await page.waitFor(`${isOpen} && ${active('reader-auth-close')}`, 'open desktop dialog, initial focus is Close');
    const desktop = await page.eval(`(() => {
      const d = ${q(DIALOG)}, r = d.getBoundingClientRect();
      return { modal: d.matches(':modal'), center: (r.left+r.right)/2, viewport: innerWidth/2,
        heading: document.getElementById(d.getAttribute('aria-labelledby')).textContent.trim() };
    })()`);
    check(desktop.modal && Math.abs(desktop.center - desktop.viewport) <= 2 && desktop.heading === 'Đăng nhập để nhận xét', '#328 native desktop dialog, centered, same shared contextual form and name');
    await page.capture('reader-auth-1280-light-100', { boundary: 'real-auth-reader-guest-desktop' });
    await page.key('Tab', 8); // Shift+Tab from the first stop wraps to the last action.
    check(await page.eval(active('use-email-code')), '#328 Shift+Tab stays in modal');
    await page.key('Tab');
    check(await page.eval(active('reader-auth-close')), '#328 Tab from the last action wraps to Close');
    await page.key('Escape');
    await page.waitFor(`!${isOpen} && ${active('root-composer-auth')}`, 'Escape returns to root trigger');
    check(await page.eval(preserved(ROOT_EDITOR, 'readerAuthRoot', ROOT_TEXT)), '#328 Escape preserves Reader, textarea identity, text, selection and window scroll');

    await page.key('Enter');
    await page.waitFor(isOpen, 'open before Back');
    await page.eval('history.back()');
    await page.waitFor(`!${isOpen} && ${active('root-composer-auth')}`, 'Back dismisses in-page overlay');
    await page.eval('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
    const historyFacts = await page.eval(`(() => {
      const e = ${q(ROOT_EDITOR)}, previous = window.readerAuthRoot;
      return { path: location.pathname, y: scrollY, before: previous.y, same: e === previous.e,
        selection: [e.selectionStart, e.selectionEnd, e.selectionDirection] };
    })()`);
    check(await page.eval(`location.pathname === '${HERE}' && ${preserved(ROOT_EDITOR, 'readerAuthRoot', ROOT_TEXT)}`), `#328 Back keeps the same route, mounted draft, selection and scroll (${JSON.stringify(historyFacts)})`);

    await page.viewport(390, 844);
    await page.eval(`document.documentElement.dataset.theme = 'dark'; ${q(ROOT_EDITOR+'-auth')}.focus();`);
    await page.key('Enter');
    await page.waitFor(isOpen, 'mobile sheet at reference viewport');
    check(await page.eval(`(() => {
      const r = ${q(DIALOG)}.getBoundingClientRect();
      return Math.abs(r.bottom-innerHeight) <= 1 && r.left === 0 && r.right === innerWidth;
    })()`), '#328 mobile sheet is pinned to the bottom edge at the approved 390×844 viewport');
    await page.capture('reader-auth-390-dark-100', { boundary: 'real-auth-reader-guest-reference-viewport' });
    await page.click('#reader-auth-close');

    await page.viewport(320, 800);
    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
    await page.eval(`document.documentElement.style.fontSize='200%'; ${q(ROOT_EDITOR+'-auth')}.focus();`);
    await page.key('Enter');
    await page.waitFor(`${isOpen} && ${active('reader-auth-close')}`, '320px/200% sheet starts at Close without scrolling past its title');
    const geometry = await page.eval(`(() => {
      const d = ${q(DIALOG)}, r = d.getBoundingClientRect(), title = ${q('#reader-auth-title')}.getBoundingClientRect();
      return d.scrollTop === 0 && d.scrollWidth <= d.clientWidth + 1 && getComputedStyle(d).overflowY === "auto" &&
        title.top >= r.top && title.bottom <= r.bottom && Math.abs(r.bottom - innerHeight) <= 1 &&
        matchMedia('(prefers-reduced-motion: reduce)').matches;
    })()`);
    check(geometry, '#328 320px/200%: heading first, scrollable vertically, no horizontal clipping, reduced motion');
    await page.capture('reader-auth-320-dark-200', { boundary: 'real-auth-reader-guest-narrow-200' });
    // Also shrink the height to exercise actual scrolling, not only the overflow style.
    await page.viewport(320, 500);
    check(await page.eval(`${q(DIALOG)}.scrollHeight > ${q(DIALOG)}.clientHeight`), '#328 short 200% viewport needs actual modal scrolling');
    const tabs = [];
    for (let i = 0; i < 8; i++) {
      await page.key('Tab');
      tabs.push(await page.eval('document.activeElement?.id'));
      if (tabs.at(-1) === 'use-email-code') break;
    }
    const hit = await page.eval(`(() => {
      const el = ${q('#use-email-code')}, r = el.getBoundingClientRect();
      return el.contains(document.elementFromPoint(r.x + r.width/2, r.y + r.height/2));
    })()`);
    check(hit && ['password-login-username', 'password-login-password', 'password-login-submit', 'use-registration', 'use-email-code'].every(id => tabs.includes(id)),
      `#328 320px/200%: form fields, primary and both alternatives reachable by Tab (${tabs.join(', ')})`);
    await page.capture('reader-auth-320-dark-200-end', { boundary: 'real-auth-reader-last-action-focused' });
    await page.key('Escape');

    await page.viewport(1280, 844);
    await page.eval("document.documentElement.style.fontSize = ''");
    await page.eval('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
    await page.click('#reply-to-' + ROOT);
    await page.waitFor(`${q(REPLY_EDITOR)} && ${active(REPLY_EDITOR.slice(1))}`, 'root-specific reply composer has its initial focus');
    await page.typeAtEnd(REPLY_EDITOR, REPLY_TEXT);
    // Use the same pointer placement as click() before recording the viewport.
    await page.eval(`${q(REPLY_EDITOR+'-auth')}.scrollIntoView({block:'center',behavior:'instant'})`);
    await page.eval('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
    const before = await snapshot(page, REPLY_EDITOR, 'readerAuthReply');
    const context = await page.eval(`${q(REPLY_EDITOR)}.closest('.vk-feedback-composer').querySelector('.vk-discussion-composer-heading p').textContent`);
    check(before.y > 0 && context.startsWith('Đang trả lời @'), '#328 starting from a nonzero scroll position and an actual public reply target');
    await page.click(REPLY_EDITOR+'-auth');
    await page.waitFor(isOpen, 'reply opens the shared Auth entry');
    const openScroll = await page.eval('({y:scrollY,maxY:document.scrollingElement.scrollHeight-innerHeight})');
    await page.click('#use-email-code');
    const address = `reader-context-${Date.now().toString(36)}@vi-du.vn`;
    await fill(page, '#sign-in-address', address);
    await page.click('#sign-in-send');
    await page.waitFor(q('#sign-in-digits'), 'real email challenge in the Reader overlay');
    const right = await mailedCode(args.outbox, address, seen);
    const wrong = String((Number(right) + 1) % 1000000).padStart(6, '0');
    await fill(page, '#sign-in-digits', wrong);
    await page.click('#sign-in-complete');
    await page.waitFor(`${q(DIALOG)}.textContent.includes('Mã chưa đúng')`, 'real wrong-code recovery');
    await page.eval('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
    const afterError = await page.eval(`(() => {
      const e = ${q(REPLY_EDITOR)}, before = window.readerAuthReply, d = ${q(DIALOG)};
      return { same: e === before.e, sameReader: ${q('#reader-paper')} === before.reader,
        textSame: e.value === ${JSON.stringify(REPLY_TEXT)}, selection: [e.selectionStart, e.selectionEnd, e.selectionDirection],
        y: scrollY, beforeY: before.y, maxY: document.scrollingElement.scrollHeight-innerHeight,
        fixed: getComputedStyle(d).position, open: d.open };
    })()`);
    check(await page.eval(`${isOpen} && ${preserved(REPLY_EDITOR, 'readerAuthReply', REPLY_TEXT)}`) && comments.count() === 0,
      `#328 rejected Auth leaves the same mounted reply and its selection intact (scroll: ${JSON.stringify({ before, openScroll, afterError })})`);

    await fill(page, '#sign-in-digits', right);
    const late = await holdAnswer(page, 'POST', '/api/v1/auth/sign-in/complete');
    await page.click('#sign-in-complete');
    check(await late.held() === 200, '#328 holds the real successful email verification before browser delivery');
    await page.click('#reader-auth-close');
    await page.waitFor(`!${isOpen} && ${active(REPLY_EDITOR.slice(1)+'-auth')}`, 'dismiss the live request; focus returns to the reply trigger');
    // Native dialog may restore focus synchronously, before the host restores scroll.
    // Let dismissal settle, then choose a stable focus target before delivering the ACK.
    await page.eval('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
    check(await page.eval(preserved(REPLY_EDITOR, 'readerAuthReply', REPLY_TEXT)), '#328 close during Auth restores reply scroll, text, and selection');
    await page.eval(`${q(REPLY_EDITOR)}.focus({preventScroll:true})`);
    await late.finish(false, true);
    await page.waitFor(`${standing('needs_principal')} && ${q(REPLY_EDITOR+'-auth')}.textContent.includes('Chọn bút danh')`, 'reconciled unbound session from a late real answer');
    const lateFacts = await page.eval(`({
      open: ${isOpen}, form: !!${q('#sign-in-digits')}, focus: document.activeElement?.id,
      private: !!${q('#own-comments')}, scroll: scrollY, preserved: ${preserved(REPLY_EDITOR, 'readerAuthReply', REPLY_TEXT)}
    })`);
    check(confirmations.count() === 2 && comments.count() === 0 && !lateFacts.open && !lateFacts.form &&
      lateFacts.focus === REPLY_EDITOR.slice(1) && !lateFacts.private && lateFacts.preserved,
      `#328 late success neither reopens nor steals focus, sends nothing, and leaves the reply intact; unbound session cannot write (${JSON.stringify(lateFacts)})`);

    await page.key('Tab'); // The next useful action in this composer is choosing a pen name.
    await page.eval(`${q(REPLY_EDITOR+'-auth')}.focus({preventScroll:true})`);
    await page.key('Enter');
    await page.waitFor(`${isOpen} && ${q('#pen-name-form')}`, 'shared server-required pen-name form in the same dialog');
    const penName = 'Mây bên hiên';
    await fill(page, '#pen-name', penName);
    const penRequests = captureRequest(page, 'POST', '/api/v1/auth/principal');
    const bind = await holdAnswer(page, 'POST', '/api/v1/auth/principal');
    await page.click('#pen-name-save');
    check([200, 201].includes(await bind.held()) && await page.eval(isOpen) && comments.count() === 0,
      '#328 actual principal write alone does not close the overlay or send the comment');
    await bind.finish(false, true);
    await page.waitFor(`!${isOpen} && ${active(REPLY_EDITOR.slice(1))} && ${q(REPLY_EDITOR+'-send')}?.getAttribute('aria-disabled') !== 'true'`, 'confirmed owner returns to the original reply editor');
    const current = await account(page);
    const returned = await page.eval(`(() => {
      const e = ${q(REPLY_EDITOR)}, before = window.readerAuthReply;
      const sameRoot = ${q(ROOT_EDITOR)} === window.readerAuthRoot.e && ${q(ROOT_EDITOR)}.value === ${JSON.stringify(ROOT_TEXT)};
      const sameReply = ${preserved(REPLY_EDITOR, 'readerAuthReply', REPLY_TEXT, false)};
      const sameContext = e.closest('.vk-feedback-composer').querySelector('.vk-discussion-composer-heading p').textContent === ${JSON.stringify(context)};
      return { active: ${standing('active')}, sameRoot, sameReply, sameContext, beforeTop: before.top,
        top: e.getBoundingClientRect().top, scroll: scrollY, focus: document.activeElement?.id };
    })()`);
    check(current.status === 200 && current.body.display_name === penName && penRequests.count() === 1 &&
      comments.count() === 0 && returned.active && returned.sameRoot && returned.sameReply && returned.sameContext &&
      returned.focus === REPLY_EDITOR.slice(1) && Math.abs(returned.top - returned.beforeTop) <= 3,
      `#328 confirmed principal preserves both mounted editors, reply context, text, selection, focus and viewport anchor with zero comment POSTs (${JSON.stringify(returned)})`);
    const posted = await holdAnswer(page, 'POST', DISCUSSION + '/comments');
    await page.click(REPLY_EDITOR+'-send');
    check(await posted.held() === 201 && comments.count() === 1, '#328 exactly one real comment POST only after explicitly pressing Gửi');
    await posted.finish(false, true);
    await page.waitFor(`${q('#own-comments')}?.textContent.includes(${JSON.stringify(REPLY_TEXT)})`, 'the explicit reply is visible only in own pending comments');
    check(await page.eval(`(() => {
      const stored = JSON.stringify([Object.entries(localStorage), Object.entries(sessionStorage), history.state, location.href]);
      return !stored.includes(${JSON.stringify(ROOT_TEXT.split('\n')[0])}) && !stored.includes(${JSON.stringify(REPLY_TEXT.split('\n')[0])});
    })()`) && page.errors.length === 0, '#328 no draft in browser storage or history and no console exceptions');
    penRequests.close();
    confirmations.close();
    comments.close();
  } finally {
    await owner.cdp.send('Target.disposeBrowserContext', { browserContextId });
  }
}
