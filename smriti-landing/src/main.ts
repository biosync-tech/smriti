// ── Neural network canvas (organic, brain-like) ────────────────

interface Neuron {
  x: number;
  y: number;
  vx: number;
  vy: number;
  r: number;
  hue: number;
}

function initCanvas() {
  const canvas = document.getElementById('neurons') as HTMLCanvasElement | null;
  if (!canvas) return;
  const ctx = canvas.getContext('2d')!;
  let W = 0, H = 0;

  function resize() {
    const dpr = window.devicePixelRatio || 1;
    const parent = canvas!.parentElement!;
    W = parent.clientWidth;
    H = parent.clientHeight;
    canvas!.width = W * dpr;
    canvas!.height = H * dpr;
    canvas!.style.width = W + 'px';
    canvas!.style.height = H + 'px';
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }
  resize();
  window.addEventListener('resize', resize);

  const COUNT = 50;
  const neurons: Neuron[] = Array.from({ length: COUNT }, () => ({
    x: Math.random(),
    y: Math.random(),
    vx: (Math.random() - 0.5) * 0.0001,
    vy: (Math.random() - 0.5) * 0.0001,
    r: 1.5 + Math.random() * 2,
    hue: Math.random() > 0.6 ? 43 : Math.random() > 0.5 ? 197 : 270, // amber, cyan, purple
  }));

  function draw() {
    ctx.clearRect(0, 0, W, H);

    // Connections
    for (let i = 0; i < neurons.length; i++) {
      for (let j = i + 1; j < neurons.length; j++) {
        const a = neurons[i], b = neurons[j];
        const dx = (a.x - b.x) * W;
        const dy = (a.y - b.y) * H;
        const dist = Math.sqrt(dx * dx + dy * dy);
        if (dist < 160) {
          const alpha = (1 - dist / 160) * 0.15;
          ctx.beginPath();
          ctx.moveTo(a.x * W, a.y * H);
          ctx.lineTo(b.x * W, b.y * H);
          ctx.strokeStyle = `hsla(43, 80%, 60%, ${alpha})`;
          ctx.lineWidth = 0.6;
          ctx.stroke();
        }
      }
    }

    // Neurons
    for (const n of neurons) {
      n.x += n.vx;
      n.y += n.vy;
      if (n.x < 0 || n.x > 1) n.vx *= -1;
      if (n.y < 0 || n.y > 1) n.vy *= -1;

      const px = n.x * W, py = n.y * H;

      // Glow
      const g = ctx.createRadialGradient(px, py, 0, px, py, n.r * 6);
      g.addColorStop(0, `hsla(${n.hue}, 70%, 65%, 0.2)`);
      g.addColorStop(1, 'transparent');
      ctx.beginPath();
      ctx.arc(px, py, n.r * 6, 0, Math.PI * 2);
      ctx.fillStyle = g;
      ctx.fill();

      // Dot
      ctx.beginPath();
      ctx.arc(px, py, n.r, 0, Math.PI * 2);
      ctx.fillStyle = `hsla(${n.hue}, 70%, 65%, 0.7)`;
      ctx.fill();
    }

    requestAnimationFrame(draw);
  }
  draw();
}

// ── Scroll reveal ──────────────────────────────────────────────

function initReveal() {
  const observer = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (e.isIntersecting) {
          (e.target as HTMLElement).classList.add('revealed');
          observer.unobserve(e.target);
        }
      }
    },
    { threshold: 0.12 }
  );
  document.querySelectorAll('[data-reveal]').forEach((el) => observer.observe(el));
}

// ── Counter animation ──────────────────────────────────────────

function initCounters() {
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const el = entry.target as HTMLElement;
        const target = parseFloat(el.dataset.count || '0');
        const unit = el.dataset.unit || '';
        const isDecimal = target % 1 !== 0;
        const duration = 1000;
        const start = performance.now();

        function tick(now: number) {
          const p = Math.min((now - start) / duration, 1);
          const eased = 1 - Math.pow(1 - p, 3);
          const val = target * eased;
          el.innerHTML = (isDecimal ? val.toFixed(1) : Math.round(val).toString()) + `<small>${unit}</small>`;
          if (p < 1) requestAnimationFrame(tick);
        }
        requestAnimationFrame(tick);
        observer.unobserve(el);
      }
    },
    { threshold: 0.5 }
  );
  document.querySelectorAll<HTMLElement>('[data-count]').forEach((el) => observer.observe(el));
}

// ── Nav scroll ─────────────────────────────────────────────────

function initNav() {
  const nav = document.getElementById('nav');
  if (!nav) return;
  const check = () => nav.classList.toggle('scrolled', window.scrollY > 50);
  window.addEventListener('scroll', check, { passive: true });
  check();
}

// ── Smooth scroll ──────────────────────────────────────────────

function initSmooth() {
  document.querySelectorAll<HTMLAnchorElement>('a[href^="#"]').forEach((a) => {
    a.addEventListener('click', (e) => {
      const t = document.querySelector(a.getAttribute('href')!);
      if (t) { e.preventDefault(); t.scrollIntoView({ behavior: 'smooth' }); }
    });
  });
}

// ── 3D card tilt (mouse-following perspective) ────────────────

interface TiltState {
  el: HTMLElement;
  rect: DOMRect;
  rafId: number | null;
}

function initTilt() {
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  if (reduceMotion) return;

  const targets = document.querySelectorAll<HTMLElement>('.card, .uc-card, .paper, .bh');
  const MAX_TILT = 6;          // degrees
  const SCALE = 1.015;
  const PERSPECTIVE = 800;

  targets.forEach((el) => {
    const state: TiltState = { el, rect: el.getBoundingClientRect(), rafId: null };

    const onEnter = () => {
      state.rect = el.getBoundingClientRect();
      el.style.transition = 'transform 0.08s linear, border-color 0.3s';
      el.style.willChange = 'transform';
    };

    const onMove = (e: MouseEvent) => {
      if (state.rafId !== null) return;
      state.rafId = requestAnimationFrame(() => {
        const cx = state.rect.left + state.rect.width / 2;
        const cy = state.rect.top + state.rect.height / 2;
        const dx = (e.clientX - cx) / (state.rect.width / 2);
        const dy = (e.clientY - cy) / (state.rect.height / 2);
        const ry = Math.max(-1, Math.min(1, dx)) * MAX_TILT;
        const rx = Math.max(-1, Math.min(1, -dy)) * MAX_TILT;
        el.style.transform = `perspective(${PERSPECTIVE}px) rotateX(${rx}deg) rotateY(${ry}deg) scale(${SCALE}) translateY(-3px)`;
        state.rafId = null;
      });
    };

    const onLeave = () => {
      if (state.rafId !== null) cancelAnimationFrame(state.rafId);
      state.rafId = null;
      el.style.transition = 'transform 0.5s cubic-bezier(0.2, 0.8, 0.2, 1), border-color 0.3s';
      el.style.transform = '';
    };

    el.addEventListener('mouseenter', onEnter);
    el.addEventListener('mousemove', onMove);
    el.addEventListener('mouseleave', onLeave);
  });
}

// ── Magnetic primary buttons ─────────────────────────────────

function initMagnetic() {
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  if (reduceMotion) return;

  const buttons = document.querySelectorAll<HTMLElement>('.btn-main');
  const STRENGTH = 0.3;

  buttons.forEach((btn) => {
    let rect: DOMRect | null = null;
    let rafId: number | null = null;

    btn.addEventListener('mouseenter', () => {
      rect = btn.getBoundingClientRect();
      btn.style.transition = 'transform 0.15s cubic-bezier(0.2, 0.8, 0.2, 1), box-shadow 0.2s, background 0.2s';
    });

    btn.addEventListener('mousemove', (e) => {
      if (!rect || rafId !== null) return;
      rafId = requestAnimationFrame(() => {
        const cx = rect!.left + rect!.width / 2;
        const cy = rect!.top + rect!.height / 2;
        const dx = (e.clientX - cx) * STRENGTH;
        const dy = (e.clientY - cy) * STRENGTH;
        btn.style.transform = `translate(${dx}px, ${dy - 2}px)`;
        rafId = null;
      });
    });

    btn.addEventListener('mouseleave', () => {
      if (rafId !== null) cancelAnimationFrame(rafId);
      rafId = null;
      btn.style.transition = 'transform 0.5s cubic-bezier(0.2, 0.8, 0.2, 1), box-shadow 0.2s, background 0.2s';
      btn.style.transform = '';
    });
  });
}

// ── Mouse-tracked spotlight on cards ─────────────────────────

function initSpotlight() {
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  if (reduceMotion) return;

  const targets = document.querySelectorAll<HTMLElement>('.card, .uc-card, .paper');

  targets.forEach((el) => {
    el.addEventListener('mousemove', (e) => {
      const rect = el.getBoundingClientRect();
      const x = ((e.clientX - rect.left) / rect.width) * 100;
      const y = ((e.clientY - rect.top) / rect.height) * 100;
      el.style.setProperty('--spot-x', `${x}%`);
      el.style.setProperty('--spot-y', `${y}%`);
    });
  });
}

// ── Early-access form (Web3Forms AJAX submit) ────────────────

interface Web3FormsResponse {
  success: boolean;
  message?: string;
}

function initEarlyAccessForm() {
  const form = document.getElementById('early-access-form') as HTMLFormElement | null;
  const success = document.getElementById('early-access-success');
  const errorEl = document.getElementById('early-access-error');
  if (!form || !success || !errorEl) return;

  const button = form.querySelector<HTMLButtonElement>('button[type="submit"]');
  const label = form.querySelector<HTMLElement>('.btn-label');
  const FETCH_TIMEOUT_MS = 10000;

  const resetButton = () => {
    if (!button) return;
    button.disabled = false;
    form.classList.remove('is-submitting');
    if (label) label.textContent = 'Request Early Access';
  };

  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    if (!button) return;

    button.disabled = true;
    form.classList.add('is-submitting');
    if (label) label.textContent = 'Sending…';
    errorEl.classList.add('is-hidden');

    const formData = new FormData(form);
    const controller = new AbortController();
    const timeoutId = window.setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);

    try {
      const res = await fetch(form.action, {
        method: 'POST',
        body: formData,
        headers: { Accept: 'application/json' },
        signal: controller.signal,
      });
      window.clearTimeout(timeoutId);
      const data = (await res.json()) as Web3FormsResponse;

      if (res.ok && data.success) {
        form.classList.add('is-hidden');
        success.classList.remove('is-hidden');
        success.scrollIntoView({ behavior: 'smooth', block: 'center' });
      } else {
        throw new Error(data.message || 'Submission failed');
      }
    } catch (err) {
      window.clearTimeout(timeoutId);
      errorEl.classList.remove('is-hidden');
      resetButton();
      console.error('Early-access submit error:', err);
    }
  });
}

// ── Init ───────────────────────────────────────────────────────

document.addEventListener('DOMContentLoaded', () => {
  initCanvas();
  initReveal();
  initCounters();
  initNav();
  initSmooth();
  initTilt();
  initMagnetic();
  initSpotlight();
  initEarlyAccessForm();
});
