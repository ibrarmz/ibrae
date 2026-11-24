<script>
  import { claude, claudeCmd, claudeElapsed, clock, fmt } from './store.svelte.js';
  import { icon } from './icons.js';

  const PHASES = ['LEER', 'EDITAR', 'PROBAR', 'LISTO'];

  const s = $derived(claude.session);
  let busy = $state(false);
  let error = $state('');

  async function hooks(action) {
    busy = true;
    error = '';
    try {
      await claudeCmd.hooks(action);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // 0 empty · 1 full · 'cur' in progress
  const bars = $derived(
    s
      ? PHASES.map((_, i) => (s.status === 'done' ? 1 : s.status !== 'error' && i === s.phase ? 'cur' : s.reached[i] ? 1 : 0))
      : [],
  );

  const waitText = $derived.by(() => {
    const m = s?.message ?? '';
    const tool = m.match(/permission to use (\w+)/i)?.[1];
    return tool ? `Claude necesita permiso para usar ${tool}` : m || 'Claude espera tu respuesta';
  });

  // Agent list replaces the centre card unless the session itself waits.
  const showAgents = $derived(!!s?.agents?.length && s.status !== 'waiting');
  const rows = $derived.by(() => {
    const list = s?.agents ?? [];
    return ['waiting', 'working', 'done'].flatMap((st) => list.filter((a) => a.status === st)).slice(0, 3);
  });
  const agentTool = (a) => (a.message ?? '').match(/permission to use (\w+)/i)?.[1]?.toUpperCase();
  const agentSub = (a) =>
    a.status === 'waiting'
      ? `PIDE PERMISO${agentTool(a) ? ` · ${agentTool(a)}` : ''}`
      : [a.label, a.detail].filter(Boolean).join(' ');
  const agentTime = (a) => fmt.short(Math.max(0, ((a.ended_at ?? clock.now) - a.started_at) / 1000));
  const nAgents = $derived(s?.agents?.length ?? 0);

  const showDiff = $derived(s?.diff && (s.label === 'EDITANDO' || s.status === 'done' || !s.detail));

  // Oldest prompt the island can answer; the rest wait their turn.
  const perm = $derived(claude.perms[0]);
  const more = $derived(claude.perms.length - 1);
  let answering = $state(0);
  let permError = $state('');
  async function answer(decision) {
    const id = perm.id;
    answering = id;
    permError = '';
    try {
      await claudeCmd.answer(id, decision);
    } catch (e) {
      permError = String(e);
      setTimeout(() => (permError = ''), 3000);
    } finally {
      answering = 0;
    }
  }
</script>

{#snippet permCard()}
  <div class="card perm">
    <div class="perm-h">
      <span class="pulse sm"></span>
      <span class="who">{perm.agent ? `AGENTE ${perm.agent}` : 'CLAUDE'} · {perm.action}</span>
      <span class="ptool">{perm.tool}</span>
      <span class="sp"></span>
      {#if more > 0}<span class="more">+{more}</span>{/if}
      {#if perm.project}<span class="dim">{perm.project}</span>{/if}
    </div>
    {#if perm.diff && (perm.diff.removed.length || perm.diff.added.length)}
      <div class="pfile">{perm.detail}<span><span class="add">+{perm.diff.add}</span>  <span class="del">−{perm.diff.del}</span></span></div>
      <div class="lines plines">
        {#each perm.diff.removed as l}<div class="ln del-bg">{l || ' '}</div>{/each}
        {#each perm.diff.added as l}<div class="ln add-bg">{l || ' '}</div>{/each}
      </div>
    {:else}
      <code class="pcmd" title={perm.detail}><span>{perm.detail || perm.tool}</span></code>
    {/if}
    <div class="perm-b">
      <span class="hint" title={perm.always ? `Siempre permite: ${perm.always}` : ''}>
        {#if permError}<span class="err">{permError}</span>{:else if perm.always}Siempre: {perm.always}{:else}O responde en la terminal{/if}
      </span>
      <button class="pb deny" disabled={answering === perm.id} onclick={() => answer('deny')}>Denegar</button>
      {#if perm.always}<button class="pb always" disabled={answering === perm.id} onclick={() => answer('always')}>Siempre</button>{/if}
      <button class="pb allow" disabled={answering === perm.id} onclick={() => answer('allow')}>Permitir</button>
    </div>
  </div>
{/snippet}

{#if !s && perm}
  <div class="solo">{@render permCard()}</div>
{:else if s}
  <div class="claude">
    <div class="left">
      <div class="bars {s.status}" aria-hidden="true"><i></i><i></i></div>
      <div class="time num">{fmt.short(claudeElapsed())}</div>
      <div class="status"><span class="sq"></span>{s.label}</div>
    </div>

    <div class="right">
      <div class="head">
        <div class="prompt" title={s.prompt}>{s.prompt || 'Sesión de Claude Code'}</div>
        <div class="meta">
          {#if showAgents}{nAgents} {nAgents === 1 ? 'agente' : 'agentes'} · {s.tools ?? 0} tools{:else}{s.project}{s.tools ? ` · ${s.tools} tools` : ''}{/if}
        </div>
      </div>

      {#if perm}
        {@render permCard()}
      {:else if s.status === 'error'}
        <div class="card wait">
          <span class="pulse"></span>
          <span><b>{s.label === 'LÍMITE' ? 'Llegaste al límite de uso' : 'Claude se detuvo'}</b><br /><span class="dim">{s.message}</span></span>
        </div>
      {:else if s.status === 'waiting'}
        <div class="card wait">
          <span class="pulse"></span>
          <span><b>{waitText}</b><br /><span class="dim">Responde en la terminal.</span></span>
        </div>
      {:else if showAgents}
        <div class="agents">
          {#each rows as a (a.id)}
            <div class="arow {a.status}">
              <span class="aico">
                {#if a.status === 'working'}<span class="spin">{@html icon('spark', 14, 2)}</span>
                {:else if a.status === 'waiting'}<span class="pulse sm"></span>
                {:else}<span class="check">✓</span>{/if}
              </span>
              <span class="akind">{a.kind || 'AGENTE'}</span>
              <span class="atask">
                <span class="at">{a.task || 'Subagente'}</span>
                <span class="as">{agentSub(a)}</span>
              </span>
              <span class="atime num">{agentTime(a)}</span>
            </div>
          {/each}
        </div>
      {:else if showDiff}
        <div class="card diff">
          <div class="diff-h">
            <span>{s.diff.file.toUpperCase()}</span>
            <span><span class="add">+{s.diff.add}</span>  <span class="del">−{s.diff.del}</span></span>
          </div>
          <div class="lines">
            {#each s.diff.removed as l}<div class="ln del-bg">{l || ' '}</div>{/each}
            {#each s.diff.added as l}<div class="ln add-bg">{l || ' '}</div>{/each}
          </div>
        </div>
      {:else}
        <div class="card action">
          <span class="label">{s.status === 'done' || s.status === 'error' ? 'ÚLTIMA ACCIÓN' : 'AHORA'}</span>
          <code>{s.detail || (s.status === 'working' ? 'Pensando…' : '—')}</code>
        </div>
      {/if}

      {#if !showAgents && !perm}<div class="phases">
        {#each PHASES as p, i}
          <div class="ph">
            <div class="track">
              {#if bars[i] === 'cur'}<div class="fill cur"></div>{:else if bars[i]}<div class="fill"></div>{/if}
            </div>
            <span class:dim={!bars[i]}>{p}</span>
          </div>
        {/each}
      </div>{/if}
    </div>
  </div>
{:else}
  <div class="empty">
    {#if claude.hooks === false}
      <span class="title">Conecta Claude Code</span>
      <span class="dim">Agrega los hooks de la isla a <code>~/.claude/settings.json</code> (se guarda un respaldo). Verás qué hace Claude y cuándo te pide permiso.</span>
      <button class="primary" disabled={busy} onclick={() => hooks('install')}>{busy ? 'Conectando…' : 'Conectar'}</button>
    {:else}
      <span class="title">Esperando a Claude Code</span>
      <span class="dim">Inicia una sesión y aparecerá aquí.</span>
      {#if claude.hooks}
        <button class="ghost" disabled={busy} onclick={() => hooks('uninstall')}>Desconectar</button>
      {/if}
    {/if}
    {#if error}<span class="err">{error}</span>{/if}
  </div>
{/if}

<style>
  .claude {
    position: absolute;
    inset: 8px 0 6px;
    display: grid;
    grid-template-columns: 170px minmax(0, 1fr);
    gap: 24px;
  }
  .left { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; }
  .bars { display: flex; gap: 14px; }
  .bars i {
    display: block;
    width: 26px;
    height: 60px;
    border-radius: 13px;
    background: #f7dcc8;
    box-shadow: 0 0 26px rgba(217, 119, 87, 0.7);
  }
  .bars.working i { animation: breathe 1.6s ease-in-out infinite; }
  .bars.working i + i { animation-delay: -0.8s; }
  .bars.waiting i { background: #ffc49a; box-shadow: 0 0 30px rgba(255, 140, 60, 0.9); animation: breathe 0.9s ease-in-out infinite; }
  .bars.error i { background: #ffc49a; opacity: 0.7; box-shadow: none; }
  .bars.done i, .bars.idle i { opacity: 0.55; box-shadow: none; }
  @keyframes breathe {
    0%, 100% { transform: scaleY(1); opacity: 1; }
    50% { transform: scaleY(0.8); opacity: 0.72; }
  }
  .time { font-size: 30px; font-weight: 500; letter-spacing: 0.02em; }
  .status { display: flex; align-items: center; gap: 7px; font-size: 11px; letter-spacing: 0.14em; color: rgba(244, 237, 230, 0.8); }
  .sq { width: 7px; height: 7px; background: var(--fg); }

  .right { display: flex; flex-direction: column; justify-content: center; gap: 10px; min-width: 0; }
  .head { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; min-width: 0; }
  .prompt {
    font-family: var(--sans);
    font-size: 16px;
    font-weight: 600;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta { font-size: 10.5px; color: var(--dim); white-space: nowrap; }

  .card {
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.3);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
    overflow: hidden;
    font-size: 12px;
    line-height: 1.6;
  }
  .diff-h { display: flex; justify-content: space-between; padding: 7px 12px; font-size: 10.5px; letter-spacing: 0.1em; color: var(--dim); }
  .add { color: #7bd88f; }
  .del { color: #ff9c94; }
  .lines { max-height: 68px; overflow: hidden; padding-bottom: 4px; }
  .ln { padding: 0 12px; white-space: pre; overflow: hidden; text-overflow: ellipsis; }
  .del-bg { background: rgba(229, 83, 75, 0.24); }
  .add-bg { background: rgba(74, 190, 110, 0.22); }
  .action { padding: 10px 12px; display: flex; flex-direction: column; gap: 4px; min-height: 64px; }
  .action code { font-family: var(--mono); font-size: 12.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .wait { padding: 12px 14px; display: flex; align-items: center; gap: 12px; min-height: 64px; font-family: var(--sans); font-size: 13px; background: rgba(255, 140, 60, 0.1); box-shadow: inset 0 0 0 1px rgba(255, 154, 85, 0.3); }
  .pulse { flex: none; width: 10px; height: 10px; border-radius: 50%; background: #ff9a55; animation: pulse 1.3s ease-out infinite; }
  @keyframes pulse {
    0% { box-shadow: 0 0 0 0 rgba(255, 154, 85, 0.7); }
    100% { box-shadow: 0 0 0 10px rgba(255, 154, 85, 0); }
  }

  .perm { padding: 10px 12px 10px; display: flex; flex-direction: column; gap: 8px; background: rgba(255, 140, 60, 0.08); box-shadow: inset 0 0 0 1px rgba(255, 154, 85, 0.32); }
  .perm-h { display: flex; align-items: center; gap: 8px; font-size: 10.5px; letter-spacing: 0.12em; white-space: nowrap; min-width: 0; }
  .who { color: #ffb585; }
  .ptool { font-family: var(--mono); letter-spacing: 0.04em; color: var(--fg); overflow: hidden; text-overflow: ellipsis; }
  .sp { flex: 1; }
  .more { padding: 1px 6px; border-radius: 6px; background: rgba(255, 154, 85, 0.2); color: #ffb585; letter-spacing: 0.04em; }
  .perm-h .dim { overflow: hidden; text-overflow: ellipsis; letter-spacing: 0.04em; }
  .pcmd {
    font-family: var(--mono);
    font-size: 12.5px;
    line-height: 1.45;
    padding: 7px 10px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.35);
  }
  /* Clamped inside the padding, so a fourth line never peeks out. */
  .pcmd span {
    white-space: pre-wrap;
    word-break: break-all;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .pfile { display: flex; justify-content: space-between; gap: 10px; font-family: var(--mono); font-size: 11.5px; }
  .plines { max-height: 50px; border-radius: 8px; background: rgba(0, 0, 0, 0.3); }
  .perm-b { display: flex; align-items: center; gap: 8px; }
  .hint { flex: 1; min-width: 0; font-size: 10.5px; color: var(--dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pb { height: 30px; padding: 0 14px; border-radius: 9px; font-family: var(--sans); font-size: 12.5px; font-weight: 600; flex: none; transition: filter 0.15s, background 0.15s; }
  .pb:hover:not(:disabled) { filter: brightness(1.12); }
  .allow { background: #fff; color: #141010; }
  .always { background: rgba(255, 255, 255, 0.1); color: var(--fg); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14); }
  .deny { background: rgba(229, 83, 75, 0.16); color: #ffb0a8; box-shadow: inset 0 0 0 1px rgba(229, 83, 75, 0.35); }
  .solo { position: absolute; inset: 0; display: flex; flex-direction: column; justify-content: center; padding: 0 40px; }

  .agents { border-radius: 12px; background: rgba(255, 255, 255, 0.04); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.07); overflow: hidden; }
  .arow { display: grid; grid-template-columns: 16px 96px minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 7px 12px; }
  .arow + .arow { border-top: 1px solid rgba(255, 255, 255, 0.06); }
  .arow.done { opacity: 0.6; }
  .arow.waiting { background: rgba(255, 140, 60, 0.08); }
  .aico { display: grid; place-items: center; color: #ff9a55; }
  .spin { display: grid; animation: spin 2.4s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .pulse.sm { width: 8px; height: 8px; }
  .check { color: #7bd88f; font-size: 13px; font-weight: 700; }
  .akind { font-family: var(--mono); font-size: 10px; letter-spacing: 0.12em; text-transform: uppercase; color: #f2b89f; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .arow.waiting .akind { color: #ffb585; }
  .atask { display: flex; flex-direction: column; min-width: 0; line-height: 1.35; }
  .at { font-family: var(--sans); font-size: 12.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .as { font-family: var(--mono); font-size: 10.5px; color: var(--dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .arow.waiting .as { color: #ff9a55; }
  .atime { font-family: var(--mono); font-size: 11px; font-variant-numeric: tabular-nums; text-align: right; }

  .phases { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; margin-top: 2px; }
  .ph { display: flex; flex-direction: column; gap: 7px; font-size: 10.5px; letter-spacing: 0.14em; }
  .track { height: 3px; border-radius: 2px; background: rgba(255, 255, 255, 0.16); overflow: hidden; }
  .fill { height: 100%; background: var(--fg); }
  .fill.cur { width: 55%; animation: shimmer 1.4s ease-in-out infinite; }
  @keyframes shimmer {
    0%, 100% { opacity: 0.45; }
    50% { opacity: 1; }
  }
  .dim { color: var(--dim); }

  .empty {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    text-align: center;
    font-family: var(--sans);
    font-size: 13px;
    padding: 0 60px;
  }
  .empty .title { font-size: 20px; font-weight: 600; }
  .empty code { font-family: var(--mono); font-size: 12px; }
  .primary { margin-top: 6px; height: 40px; padding: 0 22px; border-radius: 12px; background: #fff; color: #141010; font-family: var(--sans); font-weight: 600; }
  .ghost { margin-top: 4px; font-size: 12px; color: var(--dim); text-decoration: underline; }
  .err { color: #ff9c94; font-size: 12px; }
  button:disabled { opacity: 0.6; cursor: default; }
</style>
