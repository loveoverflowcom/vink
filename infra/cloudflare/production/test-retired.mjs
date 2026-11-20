// Replacement entry point for the retired test project, installed only after cutover.
export default {
  fetch(request) {
    return new Response(request.method === 'HEAD' ? null : 'Môi trường thử nghiệm đã ngừng hoạt động.', {
      status: 410,
      headers: { 'Content-Type':'text/plain; charset=utf-8', 'Cache-Control':'no-store',
        'X-Robots-Tag':'noindex, nofollow', 'X-Content-Type-Options':'nosniff' },
    });
  },
};
