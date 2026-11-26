// Shared Login composition on the actual Product hosts. The caller supplies the route,
// theme and text size, and decides which matched scenes to capture.
function inspectLogin() {
  const form = document.querySelector('#password-login');
  const entry = form.closest('.vk-auth-entry');
  const dialog = entry.closest('dialog');
  const find = (selector) => entry.querySelector(selector);
  const selectors = ['#password-login-username', '#password-login-password'];
  const inputs = selectors.map(selector => find(selector));
  const fields = inputs.map(input => input.closest('.vk-ui-field__control'));
  const labels = inputs.map(input => input.labels[0]);
  const divider = find('.vk-auth-divider');
  const buttons = ['#password-login-submit', '#use-email-code', '#use-registration'].map(find);
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
  const centered = (el) => Math.abs(box(el).left + box(el).width / 2 - (bounds.left + bounds.width / 2)) <= 1;
  const sameColumn = (el) => Math.abs(box(el).left - bounds.left) <= 1 && Math.abs(box(el).width - bounds.width) <= 1;
  const order = [...fields, buttons[0], divider, ...buttons.slice(1)];
  const host = background(entry);
  const frame = dialog?.getBoundingClientRect();
  return {
    host,
    order: order.every((el, i) => sameColumn(el) && (!i || (
      order[i - 1].compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING &&
      box(el).top >= box(order[i - 1]).bottom - 1
    ))),
    divider: divider.textContent.trim(),
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
        distinctFill: fill !== host, edgeContrast: contrast(edge, host),
        textContrast: contrast(getComputedStyle(label).color, fill),
        icon: glyph?.dataset.icon, iconContrast: contrast(getComputedStyle(glyph).color, fill),
        labelFits: box(label).left >= box(glyph).right && label.scrollWidth <= label.clientWidth + 1,
        native: label.htmlFor === inputs[i].id && inputs[i].getAttribute('aria-required') === 'true',
      };
    }),
    buttons: buttons.map(el => {
      const style = getComputedStyle(el);
      return { text: el.textContent.trim(), height: box(el).height, border: style.borderTopWidth,
        tonal: el.classList.contains('vk-ui-button--tonal'),
        filled: el.classList.contains('vk-ui-button--filled'),
        fill: style.backgroundColor, contrast: contrast(style.color, style.backgroundColor),
        icon: icon(el)?.dataset.icon, clip: el.scrollWidth > el.clientWidth + 1,
      };
    }),
  };
}

export async function checkLoginR3(page, check, scene) {
  const result = await page.eval(`(${inspectLogin.toString()})()`);
  check(result.order && result.divider === 'Hoặc' && result.heading && result.modalGeometry &&
    result.fields.every((f, i) => f.height >= 56 && f.outline === '1px' && f.border === '0px' &&
      f.distinctFill && f.edgeContrast >= 3 && f.textContrast >= 4.5 && f.iconContrast >= 3 &&
      f.icon === ['person-outline', 'lock'][i] && f.native && f.labelFits) &&
    result.buttons.every(b => b.height >= 48 && b.border === '0px' && b.contrast >= 4.5 && !b.clip) &&
    result.buttons[0].filled && result.buttons[1].tonal && result.buttons[2].tonal &&
    result.buttons[1].icon === 'mail' && result.buttons[2].icon === 'person-add',
    `#559 ${scene}: native one-column Login, computed surface/edge/contrast and actions (${JSON.stringify(result)})`);
  return result;
}
