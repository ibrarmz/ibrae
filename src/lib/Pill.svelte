<script>
  import { media, claude, claudeElapsed, focusRemaining, fmt, activeAgents, waitingAgent, calPill, calUntil, clock, audio, osd } from './store.svelte.js';
  import { icon } from './icons.js';
  import Cover from './Cover.svelte';
  import { waitInfo } from './pill.js';

  // Highest priority first. One activity: full layout. Two: compact, the top one on the right.
  let { acts } = $props();

  // The main session's own permission wins over a subagent's.
  // A prompt the island can answer says exactly who asks and for what.
  const wait = $derived(waitInfo({ perm: claude.perms[0], session: claude.session, waitingAgent: waitingAgent() }));
  const waitAgent = $derived(wait.agent);
  const waitTool = $derived(wait.tool);
  const nAgents = $derived(activeAgents().length);
  // The turn died on an API error (usage limit…): the pill says so instead of LISTO.
  const failed = $derived(claude.session?.status === 'error');

  const cal = $derived(calPill());

  // Volume / brightness / output notice. Unmuting shows the volume again.
  const osdView = $derived(osd.kind === 'mute' && !audio.muted ? 'volume' : osd.kind);
  const outIcon = (name) => (/aud[ií]fono|headphone|headset|buds|airpods/i.test(name) ? 'headphones' : /hdmi|displayport|monitor|tv/i.test(name) ? 'monitor' : 'speaker');
  const mins = (ms) => Math.max(1, Math.ceil(ms / 60_000));
</script>

{#snippet eq()}
  <span class="eq" aria-hidden="true"><i></i><i></i><i></i><i></i></span>
{/snippet}

{#snippet compact(kind)}
  {#if kind === 'music'}
    <span class="row"><Cover />{@render eq()}</span>
  {:else if kind === 'focus'}
    <span class="row violet">{@html icon('timer', 15, 2)}<span class="num fg">{fmt.mmss(focusRemaining())}</span></span>
  {:else if kind === 'claude-work'}
    <span class="row clay"><span class="spin">{@html icon('spark', 16, 2)}</span><span class="num fg">{fmt.short(claudeElapsed())}</span></span>
  {:else if kind === 'claude-wait'}
    <span class="row"><span class="pulse"></span><span class="tag orange">PERMISO</span></span>
  {:else if kind === 'claude-done'}
    {#if failed}<span class="row"><span class="tag orange">{claude.session.label}</span></span>
    {:else}<span class="row clay">{@html icon('check', 15, 2.2)}<span class="tag">LISTO</span></span>{/if}
  {:else if kind === 'cal-next' && cal}
    <span class="row violet">{@html icon('cal', 15, 2)}<span class="num fg">{mins(cal.ev.start - clock.now)}m</span></span>
  {:else if kind === 'cal-now' && cal}
    <span class="row"><span class="cdot" style="background:{cal.ev.color}"></span><span class="num fg">{mins(cal.ev.end - clock.now)}m</span></span>
  {/if}
{/snippet}

<div class="pill" class:alert={acts[0] === 'cal-alert'}>
  {#if acts[0] === 'osd'}
    <span class="osd" class:withcover={media.track.active && media.track.playing}>
      {#if media.track.active && media.track.playing}<Cover />{/if}
      {#if osdView === 'mute'}
        <span class="oico orange">{@html icon('mute', 17, 2)}</span>
        <span class="olabel">Silenciado</span>
        <span class="tag orange">MUTE</span>
      {:else if osdView === 'output'}
        <span class="oico blue">{@html icon(outIcon(audio.output), 17, 2)}</span>
        <span class="olabel">{audio.output || 'Salida de audio'}</span>
        <span class="tag blue">SALIDA</span>
      {:else}
        {@const bright = osdView === 'brightness'}
        {@const value = bright ? (audio.brightness ?? 0) : audio.volume}
        <span class="oico" class:amber={bright}>{@html icon(bright ? 'sun' : value < 50 ? 'volLow' : 'vol', 17, 2)}</span>
        <span class="obar" class:amber={bright}><i style="width:{value}%"></i></span>
        <span class="num oval">{value}</span>
      {/if}
    </span>
  {:else if acts[0] === 'cal-alert' && cal}
    <div class="acard">
      <span class="label violet">{cal.ev.start > clock.now ? `EMPIEZA ${calUntil(cal.ev.start - clock.now).toUpperCase()}` : 'YA EMPEZÓ'}</span>
      <span class="atitle">{cal.ev.title}</span>
      <span class="ameta num">{fmt.hm24(new Date(cal.ev.start))} – {fmt.hm24(new Date(cal.ev.end))}</span>
    </div>
    <!-- The pill doesn't take clicks: hovering opens the Calendar tab, whose card has the real button. -->
    {#if cal.ev.link}
      <span class="join">{@html icon('video', 15, 2)}Unirse</span>
    {/if}
  {:else if acts.length === 2}
    {@render compact(acts[1])}
    {@render compact(acts[0])}
  {:else if acts[0] === 'music'}
    <span class="row" title={[media.track.title, media.track.artist].filter(Boolean).join(' · ')}>
      <Cover /><span class="song">{media.track.title || media.track.app}</span>
    </span>
    {@render eq()}
  {:else if acts[0] === 'focus'}
    <span class="row violet pad">{@html icon('timer', 16, 2)}<span class="tag">FOCUS</span></span>
    <span class="num big">{fmt.mmss(focusRemaining())}</span>
  {:else if acts[0] === 'claude-work'}
    {#if nAgents}
      <span class="row clay pad"><span class="spin">{@html icon('spark', 16, 2)}</span><span class="tag">DELEGANDO</span></span>
      <span class="row">
        <span class="badge"><span class="dots"><i></i><i></i><i></i></span><span class="num">{nAgents}</span></span>
        <span class="num big">{fmt.short(claudeElapsed())}</span>
      </span>
    {:else}
      <span class="row clay pad"><span class="spin">{@html icon('spark', 16, 2)}</span><span class="tag">{claude.session?.label}</span></span>
      <span class="num big">{fmt.short(claudeElapsed())}</span>
    {/if}
  {:else if acts[0] === 'claude-wait'}
    <span class="row pad"><span class="pulse"></span><span class="msg">{#if waitAgent}{#if waitAgent.kind}Agente <span class="clay">{waitAgent.kind}</span> pide permiso{:else}Un agente pide permiso{/if}{:else}Claude pide permiso{/if}</span></span>
    <span class="tag orange nowrap">{waitTool}</span>
  {:else if (acts[0] === 'cal-next' || acts[0] === 'cal-now') && cal}
    <span class="row pad" title={cal.ev.title}>
      {#if acts[0] === 'cal-next'}
        <span class="violet ico">{@html icon('cal', 15, 2)}</span>
      {:else}
        <span class="cdot" style="background:{cal.ev.color}"></span>
      {/if}
      <span class="song">{cal.ev.title}</span>
    </span>
    {#if acts[0] === 'cal-next'}
      <span class="num big violet nowrap">en {mins(cal.ev.start - clock.now)} min</span>
    {:else}
      <span class="num big dim nowrap">quedan {mins(cal.ev.end - clock.now)} min</span>
      <i class="cbar" style="width:{Math.min(100, ((clock.now - cal.ev.start) / (cal.ev.end - cal.ev.start)) * 100)}%"></i>
    {/if}
  {:else if acts[0] === 'claude-done'}
    {#if failed}
      <span class="row pad"><span class="tag orange">{claude.session.label === 'LÍMITE' ? 'LÍMITE DE USO' : claude.session.label}</span></span>
      <span class="num big">{fmt.short(claudeElapsed())}</span>
    {:else if nAgents}
      <span class="row clay pad">{@html icon('check', 16, 2.2)}<span class="tag">LISTO</span></span>
      <span class="row clay"><span class="spin sm">{@html icon('spark', 12, 2)}</span><span class="tag">{nAgents} {nAgents === 1 ? 'AGENTE SIGUE' : 'AGENTES SIGUEN'}</span></span>
    {:else}
      <span class="row clay pad">{@html icon('check', 16, 2.2)}<span class="tag">CLAUDE TERMINÓ</span></span>
      <span class="num big">{fmt.short(claudeElapsed())}</span>
    {/if}
  {/if}
</div>

<style>
  .pill {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 0 14px 0 8px;
  }
  .pill.alert { padding: 10px 14px 12px 24px; gap: 12px; }
  .acard { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .atitle { font-family: var(--sans); font-size: 14px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .ameta { font-size: 11px; color: var(--dim); }
  .join {
    flex: none;
    height: 36px;
    padding: 0 16px;
    border-radius: 18px;
    background: var(--fg);
    color: #111;
    font-size: 12px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .cdot { width: 8px; height: 8px; border-radius: 50%; flex: none; }
  .osd { flex: 1; min-width: 0; display: flex; align-items: center; gap: 11px; padding-left: 8px; }
  .osd.withcover { padding-left: 0; }
  .oico { display: grid; flex: none; color: var(--fg); }
  .amber { color: #f5c84b; }
  .blue { color: #a9c1ff; }
  .olabel { flex: 1; min-width: 0; font-family: var(--sans); font-size: 13px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .obar { flex: 1; height: 6px; border-radius: 3px; background: rgba(244, 237, 230, 0.18); overflow: hidden; }
  .obar.amber { background: rgba(245, 200, 75, 0.2); }
  .obar i { display: block; height: 100%; border-radius: 3px; background: var(--fg); transition: width 0.12s ease-out; }
  .obar.amber i { background: #f5c84b; }
  .oval { width: 26px; text-align: right; font-size: 13px; font-weight: 600; }
  .ico { display: grid; flex: none; }
  .dim { color: var(--dim); }
  .nowrap { white-space: nowrap; flex: none; }
  .cbar { position: absolute; left: 0; bottom: 0; height: 2px; background: var(--violet); }
  .row { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .pad { padding-left: 6px; }
  .violet { color: var(--violet); }
  .clay { color: #e8916f; }
  .orange { color: #ffb585; }
  .fg { color: var(--fg); font-size: 13px; }
  .tag { font-size: 11px; letter-spacing: 0.12em; white-space: nowrap; }
  .msg { font-size: 11.5px; white-space: nowrap; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .big { font-size: 13px; }
  .song {
    min-width: 0;
    font-family: var(--sans);
    font-size: 12px;
    color: rgba(244, 237, 230, 0.9);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .badge {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 2px 7px;
    border-radius: 9px;
    background: rgba(232, 145, 111, 0.16);
    color: #e8916f;
    font-size: 12px;
  }
  .dots { display: flex; gap: 2px; }
  .dots i {
    display: block;
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: currentColor;
    animation: blink 1.2s ease-in-out infinite;
  }
  .dots i:nth-child(2) { animation-delay: 0.2s; }
  .dots i:nth-child(3) { animation-delay: 0.4s; }
  @keyframes blink { 0%, 100% { opacity: 0.25; } 50% { opacity: 1; } }

  /* Rotated icons must have an even size, otherwise their centre lands on a half pixel in the 38px pill and the icon wobbles under software rendering. */
  .spin { display: grid; animation: spin 2.4s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  .pulse {
    display: block;
    flex: none;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #ff9a55;
    animation: pulse 1.3s ease-out infinite;
  }
  @keyframes pulse {
    0% { box-shadow: 0 0 0 0 rgba(255, 154, 85, 0.7); }
    100% { box-shadow: 0 0 0 9px rgba(255, 154, 85, 0); }
  }

  .eq { display: flex; gap: 2px; align-items: center; height: 14px; }
  .eq i {
    display: block;
    width: 3px;
    height: 14px;
    border-radius: 2px;
    background: rgb(var(--tr) var(--tg) var(--tb));
    animation: eq 0.9s ease-in-out infinite;
  }
  .eq i:nth-child(2) { animation-delay: -0.3s; }
  .eq i:nth-child(3) { animation-delay: -0.6s; }
  .eq i:nth-child(4) { animation-delay: -0.15s; }
  @keyframes eq {
    0%, 100% { transform: scaleY(0.3); }
    50% { transform: scaleY(1); }
  }
</style>
