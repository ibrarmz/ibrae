<script>
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { init, media, focus, claude, clock, fmt, setClockSeconds, settings, activeAgents, waitingAgent, calPill, calCmd, update, audio, osd, sys } from './lib/store.svelte.js';
  import { icon } from './lib/icons.js';
  import Pill from './lib/Pill.svelte';
  import { pillActs, pillWidth, waitInfo } from './lib/pill.js';
  import Home from './lib/Home.svelte';
  import Claude from './lib/Claude.svelte';
  import Music from './lib/Music.svelte';
  import Focus from './lib/Focus.svelte';
  import Notes from './lib/Notes.svelte';
  import System from './lib/System.svelte';
  import Settings from './lib/Settings.svelte';
  import Calendar from './lib/Calendar.svelte';

  // Must match WIN_W in src-tauri/src/hit.rs
  const WIN_W = 760;
  // Flush with the top edge: the island grows out of the screen's frame, like a notch.
  const TOP = 0;
  const OPEN = { w: 720, h: 292 };
  const PILL_H = 38;
  // Meeting about to start: the pill becomes a small card.
  const ALERT_H = 84;

  const TABS = [
    { id: 'home', label: 'INICIO', icon: 'home' },
    { id: 'claude', label: 'CLAUDE', icon: 'spark' },
    { id: 'music', label: 'MÚSICA', icon: 'music' },
    { id: 'focus', label: 'FOCUS', icon: 'timer' },
    { id: 'notes', label: 'NOTAS', icon: 'note' },
    { id: 'sys', label: 'SISTEMA', icon: 'cpu' },
    { id: 'cal', label: 'CALENDARIO', icon: 'cal' },
    { id: 'settings', label: 'AJUSTES', icon: 'gear' },
  ];
  // Inicio and Ajustes are always there; the rest can be hidden in Ajustes.
  const visibleTabs = $derived(TABS.filter((t) => t.id === 'home' || t.id === 'settings' || settings.tabs.includes(t.id)));
  $effect(() => {
    if (!visibleTabs.some((t) => t.id === tab)) tab = 'home';
  });

  // Typing a note: don't collapse when the cursor wanders off; close on click-away instead.
  let editing = $state(false);
  let noteFocus = $state(0);
  function newNote() {
    tab = 'notes';
    noteFocus++;
  }

  // Idle: just a thin line. Grab area around it is bigger than what you see.
  const LINE = { w: 64, h: 5, hitW: 140, hitH: 14 };
  // Tucked away behind a maximized window: only a strip at the very top edge reacts.
  const EDGE = { w: 220, h: 3 };

  let open = $state(false);
  let pinned = $state(false);
  let tab = $state('home');
  // Cursor passed over without intent: fade out, clicks go to what's underneath.
  let ghost = $state(false);
  // Foreground window on this monitor is maximized / fullscreen.
  let covered = $state(false);
  let hovering = false;
  let leaveT, peekT;

  const playing = $derived(media.track.active && media.track.playing);
  const cStatus = $derived(claude.session?.status);
  // Claude (or one of its agents) waits for a permission answer.
  const asking = $derived(cStatus === 'waiting' || !!waitingAgent() || claude.perms.length > 0);

  // "Claude terminó" (or why it stopped) stays in the pill for 10 s after Stop / StopFailure.
  let doneFresh = $state(false);
  $effect(() => {
    const s = claude.session;
    const left = (s?.status === 'done' || s?.status === 'error') && s.ended_at ? 10_000 - (Date.now() - s.ended_at) : 0;
    doneFresh = left > 0;
    if (left > 0) {
      const t = setTimeout(() => (doneFresh = false), left);
      return () => clearTimeout(t);
    }
  });

  const cal = $derived(calPill());

  // Volume / brightness / output notice: takes the whole pill for osd_ms.
  let osdOn = $state(false);
  $effect(() => {
    const left = osd.until - Date.now();
    osdOn = left > 0;
    if (left > 0) {
      const t = setTimeout(() => (osdOn = false), left);
      return () => clearTimeout(t);
    }
  });

  // What the collapsed pill shows, highest priority first (max two). A meeting
  // about to start takes the whole pill (it grows into a card with "Unirse").
  const acts = $derived(
    pillActs({ asking, cal, focusRunning: focus.running, claudeStatus: cStatus, playing, doneFresh, agentsActive: activeAgents().length, osdOn }),
  );
  const alert = $derived(!open && acts[0] === 'cal-alert');
  const line = $derived(!open && acts.length === 0);
  // Claude asking for permission must be seen even behind a maximized window.
  const tucked = $derived(settings.hide_when_covered && !open && covered && !asking && !alert && !osdOn);
  // A subagent asking names itself ("Agente general-purpose pide permiso"): needs more room.
  const agentAsks = $derived(!!waitInfo({ perm: claude.perms[0], session: claude.session, waitingAgent: waitingAgent() }).agent?.kind);
  const pillW = $derived(
    pillWidth({ acts, osd: { kind: osd.kind, muted: audio.muted, playing }, agentAsks, agentsActive: activeAgents().length }),
  );
  const w = $derived(open ? OPEN.w : line ? LINE.w : pillW);
  const h = $derived(open ? OPEN.h : line ? LINE.h : alert ? ALERT_H : PILL_H);
  // Slide up leaving 2 px visible.
  const lift = $derived(tucked ? -(h + TOP - 2) : 0);

  // Left / right: the core puts the window at that screen edge and the island
  // hugs the same edge of the window; centre: centred in both.
  const anchorX = (width) =>
    settings.position === 'left' ? 0 : settings.position === 'right' ? WIN_W - width : (WIN_W - width) / 2;
  const x = $derived(anchorX(w));

  // Tell the core where the island can be grabbed (kept for Linux hover too).
  let hit = { x: 0, y: 0, w: 0, h: 0 };
  $effect(() => {
    const [hw, hh] = open ? [w, h + TOP] : tucked ? [Math.max(w, EDGE.w), EDGE.h] : line ? [LINE.hitW, LINE.hitH] : [w, h + TOP];
    hit = { x: anchorX(hw), y: 0, w: hw, h: hh };
    invoke('set_hit_rect', { ...hit, open, busy: acts.length > 0 });
  });

  // Seconds only matter for countdowns / elapsed timers or an open panel.
  $effect(() => setClockSeconds(focus.running || open || cStatus === 'working' || cStatus === 'waiting'));

  // Panels exist only while open (plus the fade-out), so a closed island holds no panel DOM.
  // Plain state + CSS classes: nothing depends on a JS transition finishing.
  let panelMounted = $state(false);
  $effect(() => {
    if (open) {
      panelMounted = true;
      return;
    }
    const t = setTimeout(() => (panelMounted = false), 300);
    return () => clearTimeout(t);
  });

  const NEUTRAL = [150, 140, 130];
  const VIOLET = [139, 92, 246];
  const CLAY = [217, 119, 87];
  const ORANGE = [255, 140, 60];
  const tint = $derived.by(() => {
    if (!open && acts[0] === 'osd') {
      if (playing && media.art.color) return media.art.color;
      if (osd.kind === 'mute' && audio.muted) return ORANGE;
      if (osd.kind === 'brightness') return [245, 200, 75];
      if (osd.kind === 'output') return [96, 140, 255];
      return [244, 237, 230];
    }
    const key = open
      ? tab
      : ({ 'claude-wait': 'wait', 'claude-work': 'claude', 'claude-done': 'claude', focus: 'focus', music: 'music', 'cal-alert': 'cal', 'cal-next': 'cal', 'cal-now': 'cal' }[acts[0]] ?? 'idle');
    if (key === 'wait') return ORANGE;
    if (key === 'claude') return asking ? ORANGE : CLAY;
    if (key === 'focus') return VIOLET;
    if (key === 'notes') return [234, 179, 8];
    if (key === 'sys') return [16, 185, 129];
    if (key === 'cal') return [96, 140, 255];
    if ((key === 'music' || key === 'home') && media.track.active && media.art.color) return media.art.color;
    return NEUTRAL;
  });
  const glow = $derived(open || acts.length ? 1 : 0.45);

  function collapse() {
    if (!pinned) open = false;
  }

  // When the island closes after being clicked or typed in, it holds the keyboard
  // focus; hand it back to the app the user was in so keystrokes don't vanish.
  $effect(() => {
    if (!open && document.hasFocus()) {
      document.activeElement?.blur?.();
      invoke('release_focus');
    }
  });

  // The core decides intent: "ghost" (just passing), "active" (rested on it / Ctrl), "away".
  function onHover(state) {
    clearTimeout(leaveT);
    ghost = state === 'ghost';
    hovering = state === 'active';
    if (state === 'active') {
      clearTimeout(peekT);
      // Meeting card showing: open where "Unirse" can be clicked; it has been seen.
      if (alert && cal) {
        tab = 'cal';
        calCmd.dismiss(cal.ev.id);
      } else if (!open && acts[0] === 'osd' && settings.tabs.includes('sys')) {
        // Where the sliders and outputs are.
        tab = 'sys';
        sys.view = 'sonido';
      } else if (!open && claude.perms.length) {
        // Where Permitir / Denegar are.
        tab = 'claude';
      }
      open = true;
    } else if (state === 'away' && open && !editing) {
      leaveT = setTimeout(collapse, 450);
    }
  }

  // Clicking another app while a note is being written closes the island.
  function onWindowBlur() {
    if (open && !hovering) collapse();
  }

  // Open briefly on its own when something needs attention (e.g. focus ended).
  function peek(target, ms = 4500) {
    if (target === 'claude' && !settings.claude_peek) return;
    tab = target;
    open = true;
    clearTimeout(peekT);
    peekT = setTimeout(() => !hovering && collapse(), ms);
  }

  // Linux: the core can't see the global cursor, so hover is detected here.
  // Pointer events can come from anywhere in the window (the compositor
  // doesn't always honour the input region), so they're checked against the
  // hit rect. Leaving is reported by the core (`pointer-left`, from GTK):
  // WebKitGTK follows each leave with a synthetic pointermove at the exit
  // point, still inside the island, so pointer events are ignored until GTK
  // says the cursor is back (`pointer-entered`).
  // No ghost fade: clicks can't pass through the island itself on Linux.
  let dwellT;
  let inside = false;
  let outside = false;
  const inHit = (e) => e.clientX >= hit.x && e.clientX <= hit.x + hit.w && e.clientY >= hit.y && e.clientY <= hit.y + hit.h;
  function domLeave() {
    clearTimeout(dwellT);
    if (!inside) return;
    inside = false;
    onHover('away');
  }
  function domHover() {
    const root = document.documentElement;
    const activate = () => {
      clearTimeout(dwellT);
      onHover('active');
    };
    const track = (e) => {
      if (outside) return;
      if (!inHit(e)) return domLeave();
      if (!inside) {
        inside = true;
        clearTimeout(leaveT);
        if (open || e.ctrlKey) return activate();
        dwellT = setTimeout(activate, acts.length ? settings.dwell_busy : settings.dwell_idle);
      } else if (!open && e.ctrlKey) activate();
    };
    root.addEventListener('pointerenter', track);
    root.addEventListener('pointermove', track);
    // A click is proof the cursor is really here.
    root.addEventListener('pointerdown', (e) => {
      outside = false;
      if (!inHit(e)) return;
      inside = true;
      if (!open) activate();
    });
    root.addEventListener('pointerleave', domLeave);
    listen('pointer-left', () => {
      outside = true;
      domLeave();
    });
    listen('pointer-entered', () => (outside = false));
  }

  onMount(() => {
    init();
    invoke('hover_native').then((native) => !native && domHover());
    invoke('covered').then((v) => (covered = v));
    window.addEventListener('blur', onWindowBlur);
    const unlisten = [
      listen('hover', (e) => onHover(e.payload)),
      listen('peek', (e) => peek(e.payload)),
      listen('covered', (e) => (covered = e.payload)),
      // Tray → Configuración: stays open longer so there's time to reach it.
      listen('open-tab', (e) => peek(e.payload, 8000)),
    ];
    return () => {
      window.removeEventListener('blur', onWindowBlur);
      unlisten.forEach((p) => p.then((off) => off()));
    };
  });
</script>

<div
  class="island"
  class:open
  class:line
  class:alert
  class:ghost={settings.ghost && ghost && !open}
  style="width:{w}px;height:{h}px;top:{TOP}px;left:{x}px;transform:translateY({lift}px);--alpha:{1 - settings.transparency / 100};--tr:{tint[0]};--tg:{tint[1]};--tb:{tint[2]};--glow:{glow}"
>
  <!-- Concave fillets where the notch meets the top edge. -->
  <i class="ear l" aria-hidden="true"></i>
  <i class="ear r" aria-hidden="true"></i>

  <div class="shell">
  <div class="glow" aria-hidden="true"></div>

  <div class="pill-layer" class:shown={!open && !line} aria-hidden={open || line}>
    {#if acts.length}<Pill {acts} />{/if}
  </div>

  <div class="body" class:shown={open} inert={!open}>
    {#if panelMounted}
      <header>
        <span>{fmt.dateTag(new Date(clock.now))}</span>
        <span class="now">{fmt.hm(new Date(clock.now))}</span>
      </header>

      <main>
        {#if tab === 'home'}
          <Home go={(t) => (tab = t)} />
        {:else if tab === 'claude'}
          <Claude />
        {:else if tab === 'music'}
          <Music />
        {:else if tab === 'notes'}
          <Notes focusRequest={noteFocus} onediting={(v) => (editing = v)} />
        {:else if tab === 'sys'}
          <System />
        {:else if tab === 'settings'}
          <Settings />
        {:else if tab === 'cal'}
          <Calendar />
        {:else}
          <Focus />
        {/if}
      </main>

      <nav>
        <button class="icon-btn" aria-label="Nueva nota rápida" onclick={newNote}>{@html icon('plus', 18)}</button>
        <div class="tabs" role="tablist">
          {#each visibleTabs as t (t.id)}
            <button
              class="tab"
              class:on={tab === t.id}
              role="tab"
              aria-selected={tab === t.id}
              aria-label={t.label}
              onclick={() => (tab = t.id)}
            >
              {@html icon(t.icon)}<span>{t.label}</span>
              {#if t.id === 'settings' && update.latest}<i class="badge-dot" aria-label="Actualización disponible"></i>{/if}
            </button>
          {/each}
        </div>
        <button
          class="icon-btn"
          class:on={pinned}
          aria-label="Fijar la isla abierta"
          aria-pressed={pinned}
          onclick={() => (pinned = !pinned)}
        >
          {@html icon('pin')}
        </button>
      </nav>
    {/if}
  </div>
  </div>
</div>

<style>
  .island {
    /* left comes from the position setting; it animates with width so the
       island stays anchored (centred, or hugging its edge) while it grows. */
    position: absolute;
    /* Background only; the transparency setting never fades the content. */
    --notch: rgb(0 0 0 / var(--alpha, 1));
    --r: 16px;
    --ear: 10px;
    transition:
      width 0.6s var(--spring),
      left 0.6s var(--spring),
      height 0.6s var(--spring),
      transform 0.45s var(--spring),
      opacity 0.15s ease,
      --tr 0.8s ease, --tg 0.8s ease, --tb 0.8s ease, --glow 0.8s ease;
  }
  .island.line { --r: 5px; --ear: 4px; }
  .island.open { --r: 30px; --ear: 14px; }
  .island.alert { --r: 28px; --ear: 12px; }
  /* Passing over it: nearly invisible so what's underneath stays readable. */
  .island.ghost { opacity: 0.15; }

  /* Square on top (it's part of the frame), rounded only at the bottom. */
  .shell {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: 0 0 var(--r) var(--r);
    background: var(--notch);
    box-shadow: 0 8px 18px -10px rgba(0, 0, 0, 0.6);
    transition: border-radius 0.5s ease;
  }
  .island.line .shell { box-shadow: inset 0 -1px 0 rgba(255, 255, 255, 0.22); }

  /* A square of notch colour with a quarter circle cut out of its outer
     bottom corner: the top edge flows into the island's side. */
  .ear {
    position: absolute;
    top: 0;
    width: var(--ear);
    height: var(--ear);
    transition: width 0.5s ease, height 0.5s ease;
  }
  .ear.l {
    right: 100%;
    background: radial-gradient(farthest-side at 0 100%, transparent calc(100% - 0.75px), var(--notch) 100%);
  }
  .ear.r {
    left: 100%;
    background: radial-gradient(farthest-side at 100% 100%, transparent calc(100% - 0.75px), var(--notch) 100%);
  }

  .glow {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background:
      radial-gradient(120% 100% at 50% 150%, rgb(var(--tr) var(--tg) var(--tb) / calc(0.4 * var(--glow))) 0%, transparent 65%);
  }

  /* Both layers always exist; only these classes change. Hidden layers are also
     visibility:hidden so they can never catch clicks or linger half-faded. */
  .pill-layer,
  .body {
    position: absolute;
    opacity: 0;
    visibility: hidden;
    pointer-events: none;
    transition:
      opacity 0.12s ease,
      visibility 0s linear 0.12s;
  }
  .pill-layer.shown,
  .body.shown {
    opacity: 1;
    visibility: visible;
    pointer-events: auto;
  }
  .pill-layer { inset: 0; }
  .pill-layer.shown { transition: opacity 0.18s ease 0.1s, visibility 0s; }

  .body {
    top: 0;
    left: 0;
    width: 720px;
    height: 292px;
    padding: 14px 22px 6px;
    display: flex;
    flex-direction: column;
    transform: scale(0.97) translateY(-4px);
  }
  .body.shown {
    transform: none;
    transition:
      opacity 0.3s ease 0.09s,
      transform 0.45s var(--spring) 0.09s,
      visibility 0s;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    height: 22px;
    font-size: 11px;
    letter-spacing: 0.08em;
    color: var(--dim);
  }
  .now { color: var(--fg); }

  main { flex: 1; position: relative; min-height: 0; }

  nav {
    height: 46px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .tabs { display: flex; gap: 2px; }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 9px;
    border-radius: 16px;
    color: var(--dim);
    font-size: 10.5px;
    letter-spacing: 0.12em;
    transition: background 0.2s, color 0.2s;
  }
  .tab span {
    max-width: 0;
    overflow: hidden;
    opacity: 0;
    white-space: nowrap;
    transition: max-width 0.35s var(--spring), opacity 0.2s;
  }
  .tab:hover { color: var(--fg); }
  .tab.on { background: rgba(255, 255, 255, 0.13); color: var(--fg); }
  .tab.on span { max-width: 90px; opacity: 1; }
  .tab { position: relative; }
  .badge-dot { position: absolute; top: 5px; right: 5px; width: 7px; height: 7px; border-radius: 50%; background: #6ee7b7; }

  .icon-btn {
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    border-radius: 16px;
    color: var(--dim);
  }
  .icon-btn:hover { background: rgba(255, 255, 255, 0.08); color: var(--fg); }
  .icon-btn.on { background: rgba(255, 255, 255, 0.13); color: var(--fg); }
</style>
