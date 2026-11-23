// Advance only this document's monotonic clock past the Desk freshness deadline.
// The real visibility listener still decides whether this scope can issue a GET.
export async function wakeDesk(page) {
  await page.send('Page.bringToFront');
  await page.eval(`(() => {
    const now = performance.now.bind(performance);
    performance.now = () => now() + 121_000;
    document.dispatchEvent(new Event('visibilitychange'));
  })()`);
}
