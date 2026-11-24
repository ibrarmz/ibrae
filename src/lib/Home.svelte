<script>
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { media, focus, claude, clock, claudeElapsed, focusRemaining, focusMins, fmt } from './store.svelte.js';
  import { icon } from './icons.js';
  import Cover from './Cover.svelte';

  let { go } = $props();
  let memory = $state(null);

  // Only measured while this panel is on screen.
  onMount(() => {
    const read = () => invoke('island_memory').then((mb) => (memory = mb));
    read();
    const id = setInterval(read, 5000);
    return () => clearInterval(id);
  });

  const now = $derived(new Date(clock.now));
  const musicLine = $derived(
    media.track.active
      ? `${media.track.title || 'Sin título'}${media.track.artist ? ' · ' + media.track.artist : ''}`
      : 'Nada sonando',
  );
  const claudeLine = $derived.by(() => {
    const s = claude.session;
    if (!s) return claude.hooks === false ? 'Sin conectar' : 'Sin sesión activa';
    if (s.status === 'waiting') return 'Pide permiso';
    const what = s.detail ? ` · ${s.detail}` : '';
    if (s.status === 'error') return s.label === 'LÍMITE' ? 'Llegó al límite de uso' : 'Se detuvo por un error';
    if (s.status === 'done') return `Terminó en ${fmt.short(claudeElapsed())}`;
    return `${s.label.charAt(0)}${s.label.slice(1).toLowerCase()}${what}`;
  });
  const focusLine = $derived(
    focus.running ? `${fmt.mmss(focusRemaining())} restantes` : `${focusMins()}:00 · listo para iniciar`,
  );
</script>

<div class="home">
  <div class="clock">
    <div class="big">{fmt.hm(now).replace(/\s?[ap]\.?\s?m\.?$/i, '')}</div>
    <div class="date">{fmt.dateLong(now)}</div>
    <div class="date">{focus.sessions} {focus.sessions === 1 ? 'sesión' : 'sesiones'} de focus hoy</div>
    {#if memory !== null}<div class="mem">Esta isla usa {memory} MB</div>{/if}
  </div>
  <div class="cards">
    <button class="card" onclick={() => go('music')}>
      <Cover size={34} radius={9} />
      <span class="txt"><span class="label">MÚSICA</span><span class="v">{musicLine}</span></span>
    </button>
    <button class="card" onclick={() => go('claude')}>
      <span class="ico clay" class:wait={claude.session?.status === 'waiting'}>{@html icon('spark', 16, 2)}</span>
      <span class="txt"><span class="label">CLAUDE</span><span class="v">{claudeLine}</span></span>
    </button>
    <button class="card" onclick={() => go('focus')}>
      <span class="ico">{@html icon('timer', 16, 2)}</span>
      <span class="txt"><span class="label">FOCUS</span><span class="v">{focusLine}</span></span>
    </button>
  </div>
</div>

<style>
  .home {
    position: absolute;
    inset: 8px 0 6px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: 24px;
  }
  .clock { display: flex; flex-direction: column; justify-content: center; gap: 8px; }
  .big { font-size: 64px; line-height: 1; font-weight: 500; letter-spacing: -0.02em; }
  .date { font-family: var(--sans); font-size: 14px; color: var(--dim); }
  .date:first-letter { text-transform: uppercase; }
  .mem { font-size: 11px; color: var(--faint); margin-top: 4px; }
  .cards { display: flex; flex-direction: column; justify-content: center; gap: 8px; }
  .card {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    text-align: left;
    padding: 10px 12px;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.06);
    transition: background 0.2s;
  }
  .card:hover { background: rgba(255, 255, 255, 0.11); }
  .ico {
    width: 34px;
    height: 34px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: 9px;
    background: rgba(139, 92, 246, 0.22);
    color: var(--violet);
  }
  .ico.clay { background: rgba(217, 119, 87, 0.22); color: #e8916f; }
  .ico.wait { background: rgba(255, 140, 60, 0.3); color: #ffb585; }
  .txt { min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .v { font-family: var(--sans); font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
