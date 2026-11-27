// #328: real Chrome, App, email-code Identity and scratch PostgreSQL. The runner seeds a
// public root and a closed publication; no application responses are fabricated here.
import { account, fill, HYDRATED, mailedCode, q } from './auth-smoke.mjs';
import { captureRequest, holdAnswer } from './auth-browser-network.mjs';
import { checkAuthR31, checkModeSwitches } from './auth-r3-smoke.mjs';
import { matchesAXButton } from './visual-evidence.mjs';

const WORK = '33333333-3333-4333-8333-333333333333';
const ROOT = '3ac70000-0000-4000-8000-000000000002';
const CLOSED = '3ac70000-0000-4000-8000-000000000003';
const HERE = `/tac-pham/${WORK}`;
const DISCUSSION = `/api/v1/works/${WORK}/discussion`;
const DIALOG = '#reader-auth';
const ROOT_EDITOR = '#root-composer';
const REPLY_EDITOR = `#reply-composer-${ROOT}`;
const WHITESPACE = '\n  ';
const ROOT_TEXT = 'Nháp nhận xét: Mưa vừa tạnh bên hiên.\nCó những chiều chỉ muốn ngồi yên.';
const REPLY_TEXT = 'Phản hồi ở đúng nhánh: chữ ữ, ợ và dòng sau.\nĐừng gửi trước khi tôi bấm.';
const active = (id) => `document.activeElement?.id === ${JSON.stringify(id)}`;
const standing = (state) => `${q(DIALOG)}?.dataset.standing === ${JSON.stringify(state)}`;
const ROOT_SEND = ROOT_EDITOR + '-send';
const REPLY_SEND = REPLY_EDITOR + '-send';
const enabled = (selector) => '(' + q(selector) + "?.dataset.state === 'enabled')";
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
  const comments = captureRequest(page, 'POST', DISCUSSION + '/comments');
  async function checkClosedComposer(editor, text) {
    await page.typeAtEnd(editor, text);
    const send = editor + '-send';
    const note = editor.slice(1) + '-send-note';
    check(await page.eval(`
      !${q('meta[name="vink-sign-in"]')} && !${q(editor+'-auth')} && !${isOpen} &&
      !${q('#password-login')} && ${q(editor)}.value === ${JSON.stringify(text)} &&
      ${q(send)}.getAttribute('aria-disabled') === 'true' &&
      ${q(send)}.getAttribute('aria-describedby')?.split(/\\s+/).includes(${JSON.stringify(note)}) &&
      !!${q(send+' svg[data-icon=lock][aria-hidden=true]')} &&
      ${q('#'+note)}.textContent.includes('Vink chưa mở đăng nhập')
    `), `#328 closed sign-in: ${editor} keeps the draft with a locked send control and its visible reason`);
  }
  await checkClosedComposer(ROOT_EDITOR, ROOT_TEXT);
  await page.click('#reply-to-' + ROOT);
  await page.waitFor(`${q(REPLY_EDITOR)} && ${active(REPLY_EDITOR.slice(1))}`, 'guest reply on disabled sign-in deployment');
  await checkClosedComposer(REPLY_EDITOR, REPLY_TEXT);
  await page.click(ROOT_SEND);
  await page.click(REPLY_SEND);
  check(await page.eval('!' + isOpen) && comments.count() === 0, '#328 sign-in disabled blocks dialog and POST for both composers');
  comments.close();
}

export async function contextualReaderAuth(owner, { args, check, openPage, seen }) {
  console.log('Reader Auth R3: local sheet/dialog with owned draft, real Identity and explicit send.');
  const { browserContextId } = await owner.cdp.send('Target.createBrowserContext');
  try {
    const page = await openPage(browserContextId);
    const comments = captureRequest(page, 'POST', DISCUSSION + '/comments');
    const confirmations = captureRequest(page, 'POST', '/api/v1/auth/sign-in/complete');
    await page.goto(args.base + '/tac-pham/' + CLOSED);
    await page.waitFor(`${HYDRATED} && ${standing('anonymous')} && ${q(ROOT_EDITOR)}`, 'closed Reader, guest confirmed');
    const closed = await page.eval(`fetch('/api/v1/works/${CLOSED}/discussion/public').then(r => r.json())`);
    check(closed.settings.comments_open === false && await page.eval(`!${q(ROOT_EDITOR+'-auth')} && !${q(ROOT_SEND)}
      && ${q(ROOT_EDITOR)}.readOnly && !${q(ROOT_EDITOR)}.disabled && !!${q('#comments-mode[data-mode-kind=author-closed] :is([role=status])')}`),
      '#328 a closed discussion keeps a selectable read-only draft and offers neither sign-in nor Send');

    // Full Web runs Discovery first, so its newer publications may fill the first page.
    // Use an actual search result for the seeded Reader rather than assuming feed order.
    await page.goto(args.base + '/?q=' + encodeURIComponent('Mưa trên phố cũ'));
    const result = `.vk-public-row__link[href^="${HERE}?"]`;
    await page.waitFor(`${HYDRATED} && ${q(result)}`, 'seeded Reader in filtered Discovery results');
    await page.click(result);
    await page.waitFor(`location.pathname === '${HERE}' && ${q(ROOT_SEND)}`, 'guest Reader SPA route');
    await page.waitFor(`${standing('anonymous')} && ${q('.vk-discussion-root')}`, 'populated discussion with a real guest session');
    check(await page.eval(`(async () => {
      const response = await fetch('${HERE}');
      const rendered = new DOMParser().parseFromString(await response.text(), 'text/html');
      return rendered.querySelector('#comments-mode')?.dataset.modeKind === 'open' &&
        rendered.querySelector('#comments-mode')?.hasAttribute('hidden') &&
        rendered.querySelector('#root-list')?.dataset.order === 'oldest-first' &&
        rendered.querySelector('#root-list')?.dataset.hasMore === 'false' &&
        !!rendered.querySelector('#root-composer-send') &&
        !rendered.querySelector('#discussion-mode-toggle, .vk-discussion-guest-banner, #root-composer-auth');
    })()`), '#328 initial SSR and hydrated guest share public policy and pagination facts');
    async function checkQuietOpen(send, viewer) {
      const facts = await page.eval(`(() => {
        const mode = ${q('#comments-mode')};
        const list = ${q('#root-list')};
        return {
          open: mode.dataset.modeKind === 'open',
          noRow: mode.getClientRects().length === 0,
          noAnnouncement: [...mode.querySelectorAll('[role=status]')].every(status => !status.textContent.trim()),
          quiet: !document.querySelector('.vk-discussion-guest-banner, .vk-discussion-mode__dot, .vk-discussion-order, .vk-discussion-tools-note, #feedback-region .vk-ui-list-end'),
          noToggle: !document.querySelector('#discussion-mode-toggle'),
          order: list.dataset.order === 'oldest-first',
          complete: list.dataset.hasMore === 'false',
        };
      })()`);
      facts.axSend = await matchesAXButton(page, send, { name: 'Gửi' });
      check(Object.values(facts).every(Boolean), `#328 ${viewer} has quiet open, correct public mode/cursor and named Send (${JSON.stringify(facts)})`);
    }
    await checkQuietOpen(ROOT_SEND, 'guest');
    for (const theme of ['light', 'dark']) {
      for (const [width, height, scale] of [[1440, 900, 100], [390, 844, 100], [320, 800, 200]]) {
        await page.viewport(width, height);
        await page.eval(`document.documentElement.dataset.theme='${theme}'; document.documentElement.style.fontSize='${scale}%';
          document.querySelector('#feedback-heading').scrollIntoView()`);
        check(await page.eval("!document.querySelector('meta[name=vink-local-discussion-fixture], #discussion-refresh, #own-comments') && document.documentElement.scrollWidth <= innerWidth + 1"),
          `#328 guest discussion fits ${width} × ${height} in ${theme} at ${scale}%`);
        await page.capture(`328-discussion-guest-${width}-${theme}-${scale}`, {
          boundary: 'real-app-postgres-guest-no-local-principal',
          requestedScene: { width, height, theme, scale }, skipSceneValidation: true,
        });
      }
    }
    await page.eval("delete document.documentElement.dataset.theme; document.documentElement.style.fontSize = ''");
    await page.viewport(1280, 844);
    await page.send('Page.bringToFront');

    const saveWrite = captureRequest(page, 'PUT', '/api/v1/me/saved-works/' + WORK);
    await page.waitFor(`${q('#save-work')}?.getAttribute('aria-disabled') !== 'true' && ${q('#save-work')}`, 'guest can open Auth from Lưu');
    await page.click('#save-work');
    await page.waitFor(`${isOpen} && ${active('reader-auth-close')}`, 'Lưu opens the R3 login dialog');
    check(await page.eval(`
      ${q('#reader-auth-title')}.textContent === 'Đăng nhập để lưu bài' &&
      ${q('#reader-auth-lead')}.textContent === 'Bài sẽ nằm trong danh sách Đã lưu riêng trên hồ sơ của bạn.' &&
      ${q(DIALOG)}.getAttribute('aria-describedby') === 'reader-auth-lead'
    `), '#559 Reader save keeps its contextual title and lead');
    const saveR3 = await checkAuthR31(page, check, 'Reader save desktop');
    await page.capture('reader-auth-save-1280-light-100', { boundary: 'real-reader-save-guest-r3', loginR3: saveR3 });
    await page.key('Escape');
    await page.waitFor(`!${isOpen} && ${active('save-work')}`, 'Escape returns focus to Lưu');
    check(saveWrite.count() === 0, '#559 opening and dismissing Auth from Lưu does not save');
    saveWrite.close();

    await page.click(ROOT_SEND);
    await page.eval(`${q(ROOT_EDITOR)}.focus()`);
    await page.key('Enter');
    await page.typeAtEnd(ROOT_EDITOR, '  ');
    check(await page.eval(`!${isOpen} && !${enabled(ROOT_SEND)} && ${q(ROOT_EDITOR)}.value === ${JSON.stringify(WHITESPACE)} &&
      ${q(ROOT_SEND)}.classList.contains('vk-ui-button--tonal')`) && comments.count() === 0,
      '#328 empty activation, focus, Enter for newline and whitespace do not open Auth or send');
    await fill(page, ROOT_EDITOR, ROOT_TEXT);
    await page.send('Input.imeSetComposition', { text: 'đ', selectionStart: 1, selectionEnd: 1 });
    await page.waitFor(`!${enabled(ROOT_SEND)} && ${q(ROOT_SEND+'-note')}?.textContent.includes('Hoàn tất nhập chữ')`,
      'uncommitted input blocks the send intent');
    await page.eval(`${q(ROOT_SEND)}.click()`);
    check(await page.eval('!' + isOpen) && comments.count() === 0, '#328 in-progress IME does not open Auth or send');
    await page.send('Input.imeSetComposition', { text: '', selectionStart: 0, selectionEnd: 0 });
    await fill(page, ROOT_EDITOR, ROOT_TEXT);
    await page.waitFor(enabled(ROOT_SEND), 'committed guest text enables Send');
    check(await page.eval(`!${isOpen} && ${q(ROOT_SEND)}.classList.contains('vk-ui-button--filled')`), '#328 typing valid text leaves the dialog closed until Send');
    await page.eval(`${q(ROOT_SEND)}.focus()`);
    await snapshot(page, ROOT_EDITOR, 'readerAuthRoot');
    await page.key('Enter');
    await page.waitFor(`${isOpen} && ${active('reader-auth-close')}`, 'open desktop dialog, initial focus is Close');
    const desktop = await page.eval(`(() => {
      const d = ${q(DIALOG)}, r = d.getBoundingClientRect();
      return { modal: d.matches(':modal'), center: (r.left+r.right)/2, viewport: innerWidth/2,
        heading: document.getElementById(d.getAttribute('aria-labelledby')).textContent.trim() };
    })()`);
    check(desktop.modal && Math.abs(desktop.center - desktop.viewport) <= 2 && desktop.heading === 'Đăng nhập để nhận xét', '#328 native desktop dialog, centered, same shared contextual form and name');
    const desktopR3 = await checkAuthR31(page, check, 'Reader comment desktop');
    await page.capture('reader-auth-1280-light-100', { boundary: 'real-auth-reader-guest-desktop', loginR3: desktopR3 });
    await page.key('Tab', 8); // Shift+Tab from the first stop wraps to the last action.
    check(await page.eval(active('use-registration')), '#328 Shift+Tab stays in modal on the last R3 action');
    await page.key('Tab');
    check(await page.eval(active('reader-auth-close')), '#328 Tab from the last action wraps to Close');
    await checkModeSwitches(page, check, 'Reader comment');
    await page.key('Escape');
    await page.waitFor(`!${isOpen} && ${active(ROOT_SEND.slice(1))}`, 'Escape returns to root trigger');
    check(await page.eval(preserved(ROOT_EDITOR, 'readerAuthRoot', ROOT_TEXT)), '#328 Escape preserves Reader, textarea identity, text, selection and window scroll');

    await page.key('Enter');
    await page.waitFor(isOpen, 'open before Back');
    await page.eval('history.back()');
    await page.waitFor(`!${isOpen} && ${active(ROOT_SEND.slice(1))}`, 'Back dismisses in-page overlay');
    await page.eval('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
    const historyFacts = await page.eval(`(() => {
      const e = ${q(ROOT_EDITOR)}, previous = window.readerAuthRoot;
      return { path: location.pathname, y: scrollY, before: previous.y, same: e === previous.e,
        selection: [e.selectionStart, e.selectionEnd, e.selectionDirection] };
    })()`);
    check(await page.eval(`location.pathname === '${HERE}' && ${preserved(ROOT_EDITOR, 'readerAuthRoot', ROOT_TEXT)}`), `#328 Back keeps the same route, mounted draft, selection and scroll (${JSON.stringify(historyFacts)})`);

    await page.key('Enter');
    await page.waitFor(isOpen, 'open before trigger fallback');
    // DOM-only focus fixture: a trigger that cannot take focus after the dialog closes.
    await page.eval(`${q(ROOT_SEND)}.style.visibility = 'hidden'`);
    await page.key('Escape');
    await page.waitFor(`!${isOpen} && ${active(ROOT_EDITOR.slice(1))}`, 'unavailable Send returns focus to the native editor');
    await page.eval(`${q(ROOT_SEND)}.style.visibility = ''; ${q(ROOT_SEND)}.focus({preventScroll:true})`);

    // Reflow the actual native modal across both themes. Keep only representative PNGs.
    await page.key('Enter');
    await page.waitFor(`${isOpen} && ${active('reader-auth-close')}`, 'open Reader for the R3 reflow matrix');
    for (const [width, height] of [[320, 800], [390, 844], [834, 1112], [1440, 900]]) {
      await page.viewport(width, height);
      for (const theme of ['light', 'dark']) for (const scale of [100, 150, 200]) {
        await page.eval(`(async () => {
          document.documentElement.dataset.theme = '${theme}';
          document.documentElement.dataset.text = '${scale === 150 ? 150 : 100}';
          document.documentElement.style.fontSize = '${scale === 200 ? '200%' : ''}';
          await document.fonts.ready;
          await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
          const modal = document.querySelector('#reader-auth');
          const transitions = modal.getAnimations({ subtree: true }).filter(animation => animation instanceof CSSTransition);
          await Promise.all(transitions.map(transition => transition.finished.catch(() => {})));
        })()`);
        const loginR3 = await checkAuthR31(page, check, `Reader comment ${width}/${theme}/${scale}`);
        const capture = (width === 390 || width === 1440) && scale === 100 || width === 320 && theme === 'dark' && scale === 200;
        const scene = `${width}-${theme}-${scale}`;
        const captureMode = async (mode, facts) => {
          if (!capture || (!mode && width === 390 && theme === 'dark')) return;
          await page.eval("document.querySelector('#reader-auth').scrollTop = 0");
          await page.capture(`reader-auth-${mode}${scene}`, {
            boundary: 'real-auth-reader-guest-r31', textScale: scale, actionsR31: facts,
          });
        };
        await captureMode('', loginR3);
        await page.click('#use-registration');
        await page.waitFor(active('registration-address'), 'Reader registration mode');
        await captureMode('registration-', await checkAuthR31(page, check, `Reader comment ${scene}`, 'registration'));
        await page.click('#use-password');
        await page.waitFor(active('password-login-username'), 'Reader returns from registration');
        await page.click('#use-email-code');
        await page.waitFor(active('sign-in-address'), 'Reader email mode');
        await captureMode('email-', await checkAuthR31(page, check, `Reader comment ${scene}`, 'email'));
        await page.click('#use-password');
        await page.waitFor(active('password-login-username'), 'Reader returns from email');
      }
    }
    await page.eval("delete document.documentElement.dataset.text");
    await page.key('Escape');

    await page.viewport(390, 844);
    await page.eval(`document.documentElement.style.fontSize = ''; document.documentElement.dataset.theme = 'dark'; ${q(ROOT_SEND)}.focus();`);
    await page.key('Enter');
    await page.waitFor(isOpen, 'mobile sheet at reference viewport');
    check(await page.eval(`(() => {
      const r = ${q(DIALOG)}.getBoundingClientRect();
      return Math.abs(r.bottom-innerHeight) <= 1 && r.left === 0 && r.right === innerWidth;
    })()`), '#328 mobile sheet is pinned to the bottom edge at the approved 390×844 viewport');
    const mobileR3 = await checkAuthR31(page, check, 'Reader comment 390/dark/100');
    await page.capture('reader-auth-390-dark-100', { boundary: 'real-auth-reader-guest-reference-viewport', loginR3: mobileR3 });

    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'forced-colors', value: 'active' }] });
    await page.key('Tab');
    const forcedPaint = await page.eval(`(() => {
      const user = document.querySelector('#password-login-username'), password = document.querySelector('#password-login-password');
      const paint = input => {
        const s = getComputedStyle(input.closest('.vk-ui-field__control'));
        return [s.outlineWidth, s.outlineStyle, s.outlineColor, s.borderTopWidth, getComputedStyle(input).outlineWidth];
      };
      return { active: document.activeElement === user, user: paint(user), password: paint(password) };
    })()`);
    check(forcedPaint.active && forcedPaint.user[0] === '2px' && forcedPaint.user[1] === 'solid' &&
      forcedPaint.password[0] === '0px' && forcedPaint.password[3] === '1px' &&
      [forcedPaint.user, forcedPaint.password].every(([, , , , inputOutline]) => inputOutline === '0px'),
      `#559 Reader forced colors retains a single system-color edge and a distinct input focus (${JSON.stringify(forcedPaint)})`);
    await page.key('Tab');
    await page.key('Tab');
    check(await page.eval(`(() => {
      const reveal = document.querySelector('#password-login-password-reveal');
      const shell = reveal.closest('.vk-ui-field__control');
      return document.activeElement === reveal && getComputedStyle(reveal).outlineWidth === '2px' &&
        getComputedStyle(shell).borderTopWidth === '1px' && getComputedStyle(shell).outlineWidth === '0px';
    })()`), '#559 password reveal owns its own focus perimeter in forced colors');
    await page.send('Emulation.setEmulatedMedia', { features: [] });
    await page.click('#reader-auth-close');

    await page.viewport(320, 800);
    await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
    await page.eval(`document.documentElement.style.fontSize='200%'; ${q(ROOT_SEND)}.focus();`);
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
      if (tabs.at(-1) === 'use-registration') break;
    }
    const hit = await page.eval(`(() => {
      const el = ${q('#use-registration')}, r = el.getBoundingClientRect();
      return el.contains(document.elementFromPoint(r.x + r.width/2, r.y + r.height/2));
    })()`);
    check(hit && ['password-login-username', 'password-login-password', 'password-login-submit', 'use-email-code', 'use-registration'].every(id => tabs.includes(id)),
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
    await page.eval(`${q(REPLY_SEND)}.scrollIntoView({block:'center',behavior:'instant'})`);
    await page.eval('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
    check(await page.eval(`!${isOpen} && ${enabled(REPLY_SEND)} && !${q(REPLY_EDITOR+'-auth')}`)
      && await matchesAXButton(page, REPLY_SEND, { name: 'Gửi' }) && comments.count() === 0,
      '#328 opening a reply and typing reaches the shared named Send without Auth or POST');
    const before = await snapshot(page, REPLY_EDITOR, 'readerAuthReply');
    const context = await page.eval(`(() => {
      const editor = ${q(REPLY_EDITOR)};
      const target = document.getElementById(editor.getAttribute('aria-describedby'));
      const byline = document.getElementById('comment-' + ${JSON.stringify(ROOT)}).querySelector('.vk-ui-byline__name');
      return {
        id: target?.id,
        label: target?.textContent.trim(),
        byline: byline.textContent.trim(),
        target: target?.closest('.vk-discussion-reply-target')?.dataset.target,
      };
    })()`);
    check(before.y > 0 && context.target === 'root' && context.label === 'Trả lời ' + context.byline,
      '#328 starting from a nonzero scroll position with the selected public root as reply target');
    await page.click(REPLY_SEND);
    await page.waitFor(isOpen, 'reply opens the shared Auth entry');
    await page.key('Escape');
    await page.waitFor(`!${isOpen} && ${active(REPLY_SEND.slice(1))}`, 'Escape returns to reply Send');
    check(await page.eval(preserved(REPLY_EDITOR, 'readerAuthReply', REPLY_TEXT)) && comments.count() === 0,
      '#328 reply Escape preserves scope, text and focus without posting');
    await page.key('Enter');
    await page.waitFor(isOpen, 'the same reply Send opens Auth again');
    const openScroll = await page.eval('({y:scrollY,maxY:document.scrollingElement.scrollHeight-innerHeight})');
    await page.click('#use-email-code');
    const address = `reader-context-${Date.now().toString(36)}@vi-du.vn`;
    await fill(page, '#sign-in-address', address);
    await page.click('#sign-in-send');
    await page.waitFor(`${q('#sign-in-digits')} && ${active('sign-in-digits')}`, 'F-11 real email challenge focuses the next field in the Reader overlay');
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
    check(await page.eval(`(() => {
      const button = ${q('#sign-in-complete')};
      const note = document.getElementById(button.getAttribute('aria-describedby'));
      return button.getAttribute('aria-busy') === 'true' && !!button.querySelector('.vk-ui-arc') &&
        note?.textContent.includes('Đang kiểm tra mã') && getComputedStyle(note).clipPath !== 'none';
    })()`), 'F-18 Reader uses the shared pending spinner with a visually hidden description');
    await page.click('#reader-auth-close');
    await page.waitFor(`!${isOpen} && ${active(REPLY_SEND.slice(1))}`, 'dismiss the live request; focus returns to the reply trigger');
    // Native dialog may restore focus synchronously, before the host restores scroll.
    // Let dismissal settle, then choose a stable focus target before delivering the ACK.
    await page.eval('new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))');
    check(await page.eval(preserved(REPLY_EDITOR, 'readerAuthReply', REPLY_TEXT)), '#328 close during Auth restores reply scroll, text, and selection');
    await page.eval(`${q(REPLY_EDITOR)}.focus({preventScroll:true})`);
    await late.finish(false, true);
    await page.waitFor(`${standing('needs_principal')} && ${enabled(REPLY_SEND)} && ${q(REPLY_SEND)}.textContent.trim() === 'Gửi'`, 'reconciled unbound session from a late real answer');
    const lateFacts = await page.eval(`({
      open: ${isOpen}, form: !!${q('#sign-in-digits')}, focus: document.activeElement?.id,
      private: !!${q('#own-comments')}, scroll: scrollY, preserved: ${preserved(REPLY_EDITOR, 'readerAuthReply', REPLY_TEXT)}
    })`);
    check(confirmations.count() === 2 && comments.count() === 0 && !lateFacts.open && !lateFacts.form &&
      lateFacts.focus === REPLY_EDITOR.slice(1) && !lateFacts.private && lateFacts.preserved,
      `#328 late success neither reopens nor steals focus, sends nothing, and leaves the reply intact; unbound session cannot write (${JSON.stringify(lateFacts)})`);

    await page.eval(`${q(REPLY_SEND)}.focus({preventScroll:true})`);
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
      const target = document.getElementById(e.getAttribute('aria-describedby'));
      const sameContext = target?.id === ${JSON.stringify(context.id)} &&
        target.textContent.trim() === ${JSON.stringify(context.label)} &&
        target.closest('.vk-discussion-reply-target')?.dataset.target === 'root';
      return { active: ${standing('active')}, sameRoot, sameReply, sameContext, beforeTop: before.top,
        top: e.getBoundingClientRect().top, scroll: scrollY, focus: document.activeElement?.id };
    })()`);
    check(current.status === 200 && current.body.display_name === penName && penRequests.count() === 1 &&
      comments.count() === 0 && returned.active && returned.sameRoot && returned.sameReply && returned.sameContext &&
      returned.focus === REPLY_EDITOR.slice(1) && Math.abs(returned.top - returned.beforeTop) <= 3,
      `#328 confirmed principal preserves both mounted editors, reply context, text, selection, focus and viewport anchor with zero comment POSTs (${JSON.stringify(returned)})`);
    await checkQuietOpen(REPLY_SEND, 'confirmed member');
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
