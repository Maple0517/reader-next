/**
 * V4 Memory Console — companion.js
 * Interactive 7-domain panel renderer for the visual companion shell.
 */

// ─── Rail data ──────────────────────────────────────────────────────────────

const DOMAINS = [
  { key: 'overview', label: '总览', meta: 'health' },
  { key: 'task', label: '任务', meta: 'running · 42%' },
  { key: 'characters', label: '角色', meta: '48 canonical' },
  { key: 'relationships', label: '关系', meta: '126 edges' },
  { key: 'knowledge', label: '知识', meta: '83 facts' },
  { key: 'map', label: '地点', meta: '21 places' },
  { key: 'quality', label: '质量', meta: '4 queues' },
];

// ─── Shared CSS (appended to <head>) ────────────────────────────────────────

const SHARED_CSS = `
.v4-console { display: grid; grid-template-rows: auto 1fr; min-height: 100vh; background: var(--v4-bg); color: var(--v4-ink); font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif; line-height: 1.45; }
.v4-top-bar { display: grid; gap: 8px; padding: 10px; border-bottom: 1px solid var(--v4-line); background: var(--v4-shell-top-bg); }
.v4-masthead-title { display: flex; align-items: center; gap: 10px; }
.v4-masthead-title strong { font-size: 14px; font-weight: 850; }
.v4-masthead-sub { color: var(--v4-muted); font-size: 12px; }
.v4-masthead-actions { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.v4-console-body { display: grid; grid-template-columns: 116px 1fr; min-height: 0; }
.v4-console-rail { border-right: 1px solid var(--v4-line); background: var(--v4-rail-bg); padding: 8px; display: grid; align-content: start; gap: 6px; }
.v4-domain-rail-item { display: grid; gap: 1px; width: 100%; min-height: 38px; padding: 6px 7px; border: 1px solid var(--v4-line); background: var(--v4-surface); text-align: left; cursor: pointer; }
.v4-domain-rail-item.active { border-color: var(--v4-accent); color: var(--v4-accent); font-weight: 850; }
.v4-domain-rail-item:hover:not(.active) { background: var(--v4-soft); }
.v4-domain-rail-label { font-size: 12px; font-weight: 800; }
.v4-domain-rail-meta { color: var(--v4-muted); font-size: 10px; }
.v4-domain-rail-item.active .v4-domain-rail-meta { color: var(--v4-accent); opacity: 0.8; }
.v4-console-main { min-width: 0; padding: 10px; background: var(--v4-bg); }
.v4-console-content { min-width: 0; }
.v4-pill { display: inline-flex; align-items: center; min-height: 24px; padding: 0 9px; border: 1px solid var(--v4-line); background: var(--v4-surface); color: var(--v4-muted); font-size: 12px; font-weight: 750; white-space: nowrap; }
.v4-pill.blue { color: var(--v4-accent); border-color: color-mix(in srgb, var(--v4-accent) 34%, var(--v4-line)); }
.v4-pill.green { color: var(--v4-success); border-color: color-mix(in srgb, var(--v4-success) 34%, var(--v4-line)); }
.v4-pill.amber { color: var(--v4-warn); border-color: color-mix(in srgb, var(--v4-warn) 34%, var(--v4-line)); }
.v4-pill.red { color: var(--v4-danger); border-color: color-mix(in srgb, var(--v4-danger) 34%, var(--v4-line)); }
.v4-action-btn { border: 1px solid var(--v4-line); background: var(--v4-surface); min-height: 28px; padding: 0 9px; font-size: 11px; font-weight: 850; color: var(--v4-ink); cursor: pointer; }
.v4-action-btn.primary { border-color: color-mix(in srgb, var(--v4-accent) 48%, var(--v4-line)); color: var(--v4-accent); background: color-mix(in srgb, var(--v4-accent) 7%, var(--v4-surface)); }
@keyframes v4-shimmer { to { transform: translateX(100%); } }
@keyframes v4-scan { 0%, 18% { transform: translateX(-100%); } 72%, 100% { transform: translateX(100%); } }
@keyframes v4-pulse { 0%, 100% { opacity: 1; transform: scale(1); } 50% { opacity: 0.62; transform: scale(0.92); } }
`;

const PANEL_CSS = `
/* ── Overview ───────────────────────────────────────── */
.v4-panel-overview { display: grid; gap: 14px; }
.v4-live-core { position: relative; overflow: hidden; border: 1px solid var(--v4-line); background: var(--v4-surface); padding: 18px 16px; }
.v4-live-core::after { content: ''; position: absolute; inset: 0; background: linear-gradient(90deg, transparent 0%, color-mix(in srgb, var(--v4-accent) 8%, transparent) 50%, transparent 100%); animation: v4-scan 4s ease-in-out infinite; pointer-events: none; }
.v4-live-core-big { font-size: 48px; font-weight: 850; color: var(--v4-accent); line-height: 1; }
.v4-live-core-row { display: flex; align-items: center; gap: 10px; margin-top: 6px; }
.v4-domain-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; }
.v4-domain-card { position: relative; overflow: hidden; border: 1px solid var(--v4-line); background: var(--v4-surface); padding: 12px; }
.v4-domain-card-spark { position: absolute; bottom: 0; left: 0; right: 0; height: 3px; }
.v4-domain-card-spark.green { background: var(--v4-success); }
.v4-domain-card-spark.blue { background: var(--v4-accent); }
.v4-domain-card-spark.amber { background: var(--v4-warn); }
.v4-domain-card-val { font-size: 22px; font-weight: 850; }
.v4-domain-card-label { font-size: 11px; color: var(--v4-muted); margin-top: 2px; }
.v4-attention-list { display: grid; gap: 6px; }
.v4-attention-item { display: flex; align-items: center; gap: 8px; padding: 8px 10px; border: 1px solid var(--v4-line); background: var(--v4-surface); font-size: 12px; }
.v4-status-dot { width: 8px; height: 8px; flex-shrink: 0; }
.v4-status-dot.blue { background: var(--v4-accent); }
.v4-status-dot.red { background: var(--v4-danger); }
.v4-status-dot.amber { background: var(--v4-warn); }
.v4-status-dot.green { background: var(--v4-success); }

/* ── Task ───────────────────────────────────────────── */
.v4-panel-task { display: grid; gap: 14px; }
.v4-task-hero { display: grid; grid-template-columns: 1fr 200px; gap: 16px; border: 1px solid var(--v4-line); background: var(--v4-surface); padding: 16px; }
.v4-task-hero-left h3 { font-size: 15px; font-weight: 850; margin-bottom: 8px; }
.v4-task-hero-left .v4-action-row { display: flex; gap: 6px; margin-top: 10px; }
.v4-progress-meter { position: relative; border: 1px solid var(--v4-line); background: var(--v4-soft); height: 100%; min-height: 80px; display: grid; align-content: center; justify-items: center; }
.v4-progress-meter .v4-progress-pct { font-size: 32px; font-weight: 850; color: var(--v4-accent); }
.v4-progress-meter .v4-progress-bar { width: 80%; height: 6px; background: var(--v4-line); margin-top: 8px; position: relative; overflow: hidden; }
.v4-progress-meter .v4-progress-bar::after { content: ''; position: absolute; top: 0; left: 0; bottom: 0; width: 42%; background: var(--v4-accent); }
.v4-progress-bar-shimmer { position: absolute; top: 0; left: 0; bottom: 0; width: 42%; overflow: hidden; }
.v4-progress-bar-shimmer::after { content: ''; position: absolute; inset: 0; background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--v4-accent) 30%, transparent), transparent); animation: v4-shimmer 1.8s infinite; }
.v4-pipeline { display: flex; gap: 0; align-items: flex-start; border: 1px solid var(--v4-line); background: var(--v4-surface); padding: 16px 14px; }
.v4-stage-large { flex: 1; display: grid; justify-items: center; gap: 4px; position: relative; }
.v4-stage-dot { width: 12px; height: 12px; border-radius: 50%; }
.v4-stage-dot.done { background: var(--v4-success); }
.v4-stage-dot.active { background: var(--v4-accent); animation: v4-pulse 1.6s ease-in-out infinite; }
.v4-stage-dot.waiting { background: var(--v4-line); }
.v4-stage-label { font-size: 10px; color: var(--v4-muted); font-weight: 750; text-align: center; }
.v4-stage-large.active .v4-stage-label { color: var(--v4-accent); font-weight: 850; }
.v4-heatmap-grid { display: grid; grid-template-columns: repeat(16, 1fr); gap: 3px; padding: 12px; border: 1px solid var(--v4-line); background: var(--v4-surface); }
.v4-heatmap-cell { aspect-ratio: 1; min-height: 14px; }
.v4-heatmap-cell.green { background: var(--v4-success); opacity: 0.7; }
.v4-heatmap-cell.blue { background: var(--v4-accent); opacity: 0.5; }
.v4-heatmap-cell.red { background: var(--v4-danger); opacity: 0.5; }
.v4-heatmap-cell.grey { background: var(--v4-line); opacity: 0.5; }

/* ── Characters ─────────────────────────────────────── */
.v4-panel-chars { display: grid; grid-template-columns: 320px 1fr; gap: 10px; min-height: 400px; }
.v4-char-directory { border: 1px solid var(--v4-line); background: var(--v4-surface); display: grid; grid-template-rows: auto 1fr; }
.v4-char-search { padding: 10px; border-bottom: 1px solid var(--v4-line); }
.v4-char-search input { width: 100%; border: 1px solid var(--v4-line); background: var(--v4-bg); padding: 6px 8px; font-size: 12px; font-weight: 700; }
.v4-char-list { display: grid; align-content: start; }
.v4-char-row { display: grid; grid-template-columns: 1fr auto; align-items: center; padding: 8px 10px; border-bottom: 1px solid var(--v4-line); cursor: pointer; gap: 6px; }
.v4-char-row:hover { background: var(--v4-soft); }
.v4-char-row.selected { background: var(--v4-inspector-bg); border-left: 3px solid var(--v4-accent); }
.v4-char-name { font-size: 12px; font-weight: 800; }
.v4-char-chips { display: flex; gap: 4px; flex-wrap: wrap; }
.v4-char-chip { font-size: 9px; font-weight: 750; padding: 1px 5px; border: 1px solid var(--v4-line); background: var(--v4-bg); }
.v4-char-inspector { border: 1px solid var(--v4-line); background: #fef8ee; padding: 14px; display: grid; gap: 10px; align-content: start; }
.v4-char-inspector h4 { font-size: 14px; font-weight: 850; }
.v4-char-aliases { font-size: 11px; color: var(--v4-muted); }
.v4-field-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
.v4-field-box { padding: 8px; border: 1px solid var(--v4-line); background: var(--v4-surface); }
.v4-field-box-label { font-size: 9px; color: var(--v4-muted); font-weight: 750; text-transform: uppercase; letter-spacing: 0.5px; }
.v4-field-box-val { font-size: 12px; font-weight: 800; margin-top: 2px; }
.v4-state-matrix { display: flex; gap: 6px; flex-wrap: wrap; }
.v4-state-tag { font-size: 10px; font-weight: 750; padding: 2px 6px; }
.v4-identity-thread { padding: 10px; background: color-mix(in srgb, var(--v4-purple) 10%, var(--v4-surface)); border-left: 3px solid var(--v4-purple); }
.v4-identity-thread-title { font-size: 10px; font-weight: 800; color: var(--v4-purple); margin-bottom: 4px; }
.v4-evidence-block { padding: 10px; border-left: 3px solid var(--v4-success); background: var(--v4-surface); }
.v4-evidence-block-title { font-size: 10px; font-weight: 800; color: var(--v4-success); margin-bottom: 4px; }
.v4-evidence-line { font-size: 11px; color: var(--v4-muted); }

/* ── Relationships ──────────────────────────────────── */
.v4-panel-rel { display: grid; grid-template-columns: 1fr 280px; gap: 10px; min-height: 400px; }
.v4-graph-panel { position: relative; border: 1px solid var(--v4-line); background: var(--v4-surface); min-height: 360px; overflow: hidden; background-image: linear-gradient(var(--v4-line) 1px, transparent 1px), linear-gradient(90deg, var(--v4-line) 1px, transparent 1px); background-size: 40px 40px; }
.v4-graph-node { position: absolute; padding: 6px 10px; border: 1px solid var(--v4-line); background: var(--v4-surface); font-size: 11px; font-weight: 800; cursor: pointer; z-index: 2; }
.v4-graph-node:hover { border-color: var(--v4-accent); color: var(--v4-accent); }
.v4-graph-edge { position: absolute; height: 2px; background: var(--v4-line); transform-origin: 0 0; z-index: 1; }
.v4-graph-edge-label { position: absolute; font-size: 9px; color: var(--v4-muted); background: var(--v4-surface); padding: 0 4px; z-index: 3; white-space: nowrap; }
.v4-rel-inspector { border: 1px solid var(--v4-line); background: var(--v4-surface); padding: 14px; display: grid; gap: 10px; align-content: start; }
.v4-rel-inspector h4 { font-size: 13px; font-weight: 850; }
.v4-rel-detail { font-size: 11px; color: var(--v4-muted); }
.v4-rel-confidence { display: flex; align-items: center; gap: 6px; }
.v4-rel-conf-bar { flex: 1; height: 6px; background: var(--v4-line); position: relative; }
.v4-rel-conf-bar::after { content: ''; position: absolute; top: 0; left: 0; bottom: 0; width: 87%; background: var(--v4-success); }

/* ── Knowledge ──────────────────────────────────────── */
.v4-panel-knowledge { display: grid; grid-template-columns: 1fr 300px; gap: 10px; min-height: 400px; }
.v4-knowledge-atlas { border: 1px solid var(--v4-line); background: var(--v4-surface); padding: 12px; display: grid; gap: 10px; align-content: start; }
.v4-cat-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 6px; }
.v4-cat-card { padding: 10px; border: 1px solid var(--v4-line); background: var(--v4-bg); text-align: center; }
.v4-cat-card-val { font-size: 20px; font-weight: 850; }
.v4-cat-card-label { font-size: 10px; color: var(--v4-muted); }
.v4-topic-list { display: grid; gap: 4px; }
.v4-topic-row { display: flex; justify-content: space-between; align-items: center; padding: 7px 10px; border: 1px solid var(--v4-line); background: var(--v4-bg); cursor: pointer; }
.v4-topic-row:hover { background: var(--v4-soft); }
.v4-topic-name { font-size: 12px; font-weight: 800; }
.v4-topic-count { font-size: 10px; color: var(--v4-muted); }
.v4-knowledge-inspector { border: 1px solid var(--v4-line); background: var(--v4-surface); padding: 14px; display: grid; gap: 10px; align-content: start; }
.v4-knowledge-inspector h4 { font-size: 13px; font-weight: 850; }
.v4-assertion-timeline { display: grid; gap: 4px; }
.v4-assertion-item { display: flex; gap: 8px; font-size: 11px; padding: 6px 8px; border-left: 2px solid var(--v4-line); }
.v4-assertion-chapter { color: var(--v4-muted); font-weight: 750; flex-shrink: 0; }

/* ── Map ─────────────────────────────────────────────── */
.v4-panel-map { display: grid; grid-template-columns: 1fr 280px; gap: 10px; min-height: 400px; }
.v4-map-area { position: relative; border: 1px solid var(--v4-line); background: linear-gradient(135deg, #e8e0d4 0%, #d8cfc0 50%, #c8bfad 100%); min-height: 360px; overflow: hidden; }
.v4-map-pin { position: absolute; width: 10px; height: 10px; border: 2px solid var(--v4-accent); background: var(--v4-surface); border-radius: 50%; z-index: 2; cursor: pointer; }
.v4-map-pin:hover { background: var(--v4-accent); }
.v4-map-pin-label { position: absolute; font-size: 9px; font-weight: 800; white-space: nowrap; z-index: 3; pointer-events: none; }
.v4-map-route { position: absolute; height: 2px; background: var(--v4-muted); opacity: 0.4; z-index: 1; transform-origin: 0 0; }
.v4-map-inspector { border: 1px solid var(--v4-line); background: var(--v4-surface); padding: 14px; display: grid; gap: 10px; align-content: start; }
.v4-map-inspector h4 { font-size: 13px; font-weight: 850; }
.v4-map-chapter-range { font-size: 11px; color: var(--v4-muted); }
.v4-map-linked-facts { display: grid; gap: 4px; }
.v4-map-fact { font-size: 11px; padding: 5px 8px; border-left: 2px solid var(--v4-accent); background: var(--v4-bg); }

/* ── Quality ────────────────────────────────────────── */
.v4-panel-quality { display: grid; grid-template-columns: 200px 1fr 300px; gap: 10px; min-height: 400px; }
.v4-queue-col { border: 1px solid var(--v4-line); background: var(--v4-surface); display: grid; align-content: start; }
.v4-queue-header { padding: 10px; border-bottom: 1px solid var(--v4-line); font-size: 12px; font-weight: 850; }
.v4-queue-row { display: flex; justify-content: space-between; align-items: center; padding: 8px 10px; border-bottom: 1px solid var(--v4-line); cursor: pointer; }
.v4-queue-row:hover { background: var(--v4-soft); }
.v4-queue-row.selected { background: var(--v4-inspector-bg); }
.v4-queue-name { font-size: 11px; font-weight: 800; }
.v4-queue-count { font-size: 11px; color: var(--v4-muted); }
.v4-triage-col { border: 1px solid var(--v4-line); background: var(--v4-surface); display: grid; align-content: start; }
.v4-triage-header { padding: 10px; border-bottom: 1px solid var(--v4-line); font-size: 12px; font-weight: 850; }
.v4-triage-row { display: grid; grid-template-columns: 1fr auto; gap: 6px; align-items: center; padding: 8px 10px; border-bottom: 1px solid var(--v4-line); }
.v4-triage-name { font-size: 11px; font-weight: 750; }
.v4-severity-badge { font-size: 9px; font-weight: 850; padding: 1px 6px; }
.v4-severity-badge.H { background: color-mix(in srgb, var(--v4-danger) 15%, var(--v4-surface)); color: var(--v4-danger); border: 1px solid color-mix(in srgb, var(--v4-danger) 34%, var(--v4-line)); }
.v4-severity-badge.M { background: color-mix(in srgb, var(--v4-warn) 15%, var(--v4-surface)); color: var(--v4-warn); border: 1px solid color-mix(in srgb, var(--v4-warn) 34%, var(--v4-line)); }
.v4-severity-badge.L { background: color-mix(in srgb, var(--v4-success) 15%, var(--v4-surface)); color: var(--v4-success); border: 1px solid color-mix(in srgb, var(--v4-success) 34%, var(--v4-line)); }
.v4-quality-inspector { border: 1px solid var(--v4-line); background: var(--v4-surface); padding: 14px; display: grid; gap: 10px; align-content: start; }
.v4-quality-inspector h4 { font-size: 13px; font-weight: 850; }
.v4-error-code { padding: 10px; background: var(--v4-bg); border: 1px solid var(--v4-line); font-family: "SF Mono", "Fira Code", monospace; font-size: 11px; line-height: 1.5; overflow-x: auto; white-space: pre; }
.v4-quality-actions { display: flex; gap: 6px; margin-top: 4px; }
`;

// ─── Top bar ────────────────────────────────────────────────────────────────

function renderTopBar() {
  return `
    <div class="v4-masthead-title">
      <strong>《长夜记》 · V4 Memory Console</strong>
      <span class="v4-masthead-sub">墨渊 · V4 安全资料面板</span>
    </div>
    <div class="v4-masthead-actions">
      <span class="v4-pill">已读 第 43 章</span>
      <span class="v4-pill">已处理 第 42 章</span>
      <span class="v4-pill blue">当前 第 43 章</span>
      <button class="v4-action-btn">刷新状态</button>
      <button class="v4-action-btn primary">补齐到当前阅读</button>
    </div>`;
}

// ─── Rail ───────────────────────────────────────────────────────────────────

function renderRail(activeKey) {
  return DOMAINS.map(d => {
    const active = d.key === activeKey ? ' active' : '';
    return `
      <button class="v4-domain-rail-item${active}" data-domain="${d.key}">
        <span class="v4-domain-rail-label">${d.label}</span>
        <span class="v4-domain-rail-meta">${d.meta}</span>
      </button>`;
  }).join('');
}

// ─── Panel: Overview ────────────────────────────────────────────────────────

function panelOverview() {
  return `
  <div class="v4-panel-overview">
    <!-- Live Core -->
    <div class="v4-live-core">
      <div style="font-size:11px;font-weight:800;color:var(--v4-muted);margin-bottom:4px;">LIVE CORE</div>
      <div class="v4-live-core-big">42%</div>
      <div class="v4-live-core-row">
        <div style="flex:1;height:6px;background:var(--v4-line);position:relative;overflow:hidden;">
          <div style="position:absolute;top:0;left:0;bottom:0;width:42%;background:var(--v4-accent);"></div>
          <div style="position:absolute;top:0;left:0;bottom:0;width:42%;overflow:hidden;">
            <div style="position:absolute;inset:0;background:linear-gradient(90deg,transparent,color-mix(in srgb,var(--v4-accent) 30%,transparent),transparent);animation:v4-shimmer 1.8s infinite;"></div>
          </div>
        </div>
        <span class="v4-pill blue">Domain Decision</span>
      </div>
    </div>

    <!-- Domain Health -->
    <div style="font-size:11px;font-weight:800;color:var(--v4-muted);margin-bottom:2px;">DOMAIN HEALTH</div>
    <div class="v4-domain-grid">
      <div class="v4-domain-card">
        <div class="v4-domain-card-val">48</div>
        <div class="v4-domain-card-label">角色</div>
        <div class="v4-domain-card-spark green"></div>
      </div>
      <div class="v4-domain-card">
        <div class="v4-domain-card-val">126</div>
        <div class="v4-domain-card-label">关系</div>
        <div class="v4-domain-card-spark blue"></div>
      </div>
      <div class="v4-domain-card">
        <div class="v4-domain-card-val">83</div>
        <div class="v4-domain-card-label">知识</div>
        <div class="v4-domain-card-spark amber"></div>
      </div>
      <div class="v4-domain-card">
        <div class="v4-domain-card-val">21</div>
        <div class="v4-domain-card-label">地点</div>
        <div class="v4-domain-card-spark green"></div>
      </div>
    </div>

    <!-- Attention List -->
    <div style="font-size:11px;font-weight:800;color:var(--v4-muted);margin-bottom:2px;">ATTENTION</div>
    <div class="v4-attention-list">
      <div class="v4-attention-item"><span class="v4-status-dot blue"></span> 3 条新关系待确认</div>
      <div class="v4-attention-item"><span class="v4-status-dot red"></span> 1 条知识出现冲突</div>
      <div class="v4-attention-item"><span class="v4-status-dot amber"></span> 2 个角色存在别名歧义</div>
      <div class="v4-attention-item"><span class="v4-status-dot green"></span> 所有地点已拓扑映射</div>
    </div>
  </div>`;
}

// ─── Panel: Task ────────────────────────────────────────────────────────────

function panelTask() {
  const stages = [
    { name: '分章', done: true },
    { name: '提取', done: true },
    { name: '聚合', done: false, active: true },
    { name: '投射', done: false },
    { name: '校验', done: false },
  ];

  const stageNodes = stages.map(s => {
    let cls = 'waiting';
    if (s.done) cls = 'done';
    if (s.active) cls = 'active';
    const activeCls = s.active ? ' active' : '';
    return `
      <div class="v4-stage-large${activeCls}">
        <div class="v4-stage-dot ${cls}"></div>
        <div class="v4-stage-label">${s.name}</div>
      </div>`;
  }).join('');

  const heatmapColors = ['green', 'green', 'green', 'green', 'blue', 'green', 'green', 'green',
    'blue', 'blue', 'green', 'green', 'grey', 'grey', 'green', 'green',
    'green', 'red', 'green', 'green', 'blue', 'green', 'grey', 'green',
    'green', 'green', 'green', 'blue', 'green', 'green', 'red', 'green'];
  const heatmapCells = heatmapColors.map(c => `<div class="v4-heatmap-cell ${c}"></div>`).join('');

  return `
  <div class="v4-panel-task">
    <!-- Task Hero -->
    <div class="v4-task-hero">
      <div class="v4-task-hero-left">
        <h3>第 43 章 · 增量分析</h3>
        <div style="font-size:12px;color:var(--v4-muted);">started 12s ago · est. 17s remaining</div>
        <div class="v4-action-row">
          <button class="v4-action-btn">暂停</button>
          <button class="v4-action-btn primary">跳到校验</button>
        </div>
      </div>
      <div class="v4-progress-meter">
        <div class="v4-progress-pct">42%</div>
        <div class="v4-progress-bar">
          <div class="v4-progress-bar-shimmer"></div>
        </div>
      </div>
    </div>

    <!-- Pipeline Timeline -->
    <div class="v4-pipeline">${stageNodes}</div>

    <!-- Mini Heatmap -->
    <div style="font-size:10px;font-weight:800;color:var(--v4-muted);margin-bottom:2px;">CHAPTER ACTIVITY</div>
    <div class="v4-heatmap-grid">${heatmapCells}</div>
  </div>`;
}

// ─── Panel: Characters ──────────────────────────────────────────────────────

function panelCharacters() {
  const chars = [
    { name: '白无常', chips: ['主角', '阳界'], selected: true },
    { name: '李青萝', chips: ['配角', '人间'], selected: false },
    { name: '红衣女子', chips: ['反派', '阴界'], selected: false },
    { name: '祁观', chips: ['配角', '阳界'], selected: false },
  ];

  const charRows = chars.map(c => `
    <div class="v4-char-row${c.selected ? ' selected' : ''}">
      <span class="v4-char-name">${c.name}</span>
      <span class="v4-char-chips">${c.chips.map(ch => `<span class="v4-char-chip">${ch}</span>`).join('')}</span>
    </div>`).join('');

  return `
  <div class="v4-panel-chars">
    <!-- Directory -->
    <div class="v4-char-directory">
      <div class="v4-char-search">
        <input type="text" placeholder="搜索角色…" />
      </div>
      <div class="v4-char-list">${charRows}</div>
    </div>

    <!-- Inspector -->
    <div class="v4-char-inspector">
      <h4>白无常</h4>
      <div class="v4-char-aliases">别名: 无常, 白衣使者</div>
      <div class="v4-field-grid">
        <div class="v4-field-box">
          <div class="v4-field-box-label">首次出现</div>
          <div class="v4-field-box-val">第 2 章</div>
        </div>
        <div class="v4-field-box">
          <div class="v4-field-box-label">阵营</div>
          <div class="v4-field-box-val">阳界</div>
        </div>
        <div class="v4-field-box">
          <div class="v4-field-box-label">关系数</div>
          <div class="v4-field-box-val">12</div>
        </div>
        <div class="v4-field-box">
          <div class="v4-field-box-label">提及频率</div>
          <div class="v4-field-box-val">高频</div>
        </div>
      </div>
      <div class="v4-state-matrix">
        <span class="v4-state-tag v4-pill green">存活</span>
        <span class="v4-state-tag v4-pill blue">活跃</span>
        <span class="v4-state-tag v4-pill">已登场</span>
      </div>
      <div class="v4-identity-thread">
        <div class="v4-identity-thread-title">IDENTITY THREAD</div>
        <div style="font-size:11px;">身份线索: 阳界巡查使者 → 被派往阴界调查 → 发现隐藏阴谋</div>
      </div>
      <div class="v4-evidence-block">
        <div class="v4-evidence-block-title">EVIDENCE</div>
        <div class="v4-evidence-line">Ch.2: "白无常站在城门前,审视着来往的亡魂。"</div>
        <div class="v4-evidence-line">Ch.5: "他轻拂衣袖,令牌在掌心浮现幽蓝光芒。"</div>
        <div class="v4-evidence-line">Ch.18: "白无常与红衣女子的对话揭开了阴界的秘密。"</div>
      </div>
    </div>
  </div>`;
}

// ─── Panel: Relationships ───────────────────────────────────────────────────

function panelRelationships() {
  const nodes = [
    { name: '白无常', x: 60, y: 40 },
    { name: '李青萝', x: 240, y: 30 },
    { name: '红衣女子', x: 180, y: 180 },
    { name: '祁观', x: 320, y: 140 },
    { name: '黑判官', x: 100, y: 280 },
    { name: '城隍', x: 300, y: 280 },
  ];

  const edges = [
    { from: 0, to: 1, label: '盟友' },
    { from: 0, to: 2, label: '对立' },
    { from: 1, to: 3, label: '师徒' },
    { from: 2, to: 4, label: '操控' },
    { from: 3, to: 5, label: '下属' },
    { from: 0, to: 4, label: '调查' },
  ];

  const nodeMap = {};
  nodes.forEach((n, i) => { nodeMap[i] = n; });

  const nodeEls = nodes.map((n, i) =>
    `<div class="v4-graph-node" style="left:${n.x}px;top:${n.y}px;">${n.name}</div>`
  ).join('');

  const edgeEls = edges.map(e => {
    const a = nodeMap[e.from];
    const b = nodeMap[e.to];
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    const len = Math.sqrt(dx * dx + dy * dy);
    const angle = Math.atan2(dy, dx) * (180 / Math.PI);
    const mx = a.x + dx / 2;
    const my = a.y + dy / 2;
    return `
      <div class="v4-graph-edge" style="left:${a.x + 40}px;top:${a.y + 12}px;width:${len}px;transform:rotate(${angle}deg);"></div>
      <div class="v4-graph-edge-label" style="left:${mx}px;top:${my - 12}px;">${e.label}</div>`;
  }).join('');

  return `
  <div class="v4-panel-rel">
    <!-- Graph -->
    <div class="v4-graph-panel">
      ${edgeEls}
      ${nodeEls}
    </div>

    <!-- Inspector -->
    <div class="v4-rel-inspector">
      <h4>白无常 → 红衣女子</h4>
      <div class="v4-rel-detail">关系类型: 对立</div>
      <div class="v4-rel-detail">方向: 双向</div>
      <div style="font-size:10px;font-weight:800;color:var(--v4-muted);margin-top:4px;">CONFIDENCE</div>
      <div class="v4-rel-confidence">
        <div class="v4-rel-conf-bar"></div>
        <span style="font-size:12px;font-weight:850;">87%</span>
      </div>
      <div style="font-size:10px;font-weight:800;color:var(--v4-muted);margin-top:4px;">EVENTS</div>
      <div style="font-size:12px;font-weight:850;">5 events</div>
      <div style="font-size:11px;color:var(--v4-muted);margin-top:2px;">
        Ch.3 · 初次相遇<br/>
        Ch.12 · 激烈交锋<br/>
        Ch.18 · 阴界对话<br/>
        Ch.29 · 阵前对峙<br/>
        Ch.41 · 秘密联盟
      </div>
    </div>
  </div>`;
}

// ─── Panel: Knowledge ───────────────────────────────────────────────────────

function panelKnowledge() {
  const categories = [
    { val: 28, label: '世界观' },
    { val: 15, label: '能力' },
    { val: 22, label: '历史' },
    { val: 10, label: '物品' },
    { val: 5, label: '组织' },
    { val: 3, label: '规则' },
  ];

  const catEls = categories.map(c => `
    <div class="v4-cat-card">
      <div class="v4-cat-card-val">${c.val}</div>
      <div class="v4-cat-card-label">${c.label}</div>
    </div>`).join('');

  const topics = [
    { name: '阴阳界通道', count: 7 },
    { name: '轮回法则', count: 5 },
    { name: '阴界权力结构', count: 4 },
    { name: '灵力体系', count: 6 },
    { name: '历史: 百年前大战', count: 3 },
  ];

  const topicEls = topics.map(t => `
    <div class="v4-topic-row">
      <span class="v4-topic-name">${t.name}</span>
      <span class="v4-topic-count">${t.count} facts</span>
    </div>`).join('');

  return `
  <div class="v4-panel-knowledge">
    <!-- Atlas -->
    <div class="v4-knowledge-atlas">
      <div style="font-size:10px;font-weight:800;color:var(--v4-muted);">CATEGORIES</div>
      <div class="v4-cat-grid">${catEls}</div>
      <div style="font-size:10px;font-weight:800;color:var(--v4-muted);margin-top:6px;">TOPICS</div>
      <div class="v4-topic-list">${topicEls}</div>
    </div>

    <!-- Inspector -->
    <div class="v4-knowledge-inspector">
      <h4>阴阳界通道</h4>
      <div style="font-size:11px;color:var(--v4-muted);">7 assertions · first seen Ch.1</div>
      <div style="font-size:10px;font-weight:800;color:var(--v4-muted);margin-top:4px;">ASSERTION TIMELINE</div>
      <div class="v4-assertion-timeline">
        <div class="v4-assertion-item">
          <span class="v4-assertion-chapter">Ch.1</span>
          <span>通道存在于特定地点,需灵力激活</span>
        </div>
        <div class="v4-assertion-item">
          <span class="v4-assertion-chapter">Ch.8</span>
          <span>通道有时间窗口限制,仅子时可开</span>
        </div>
        <div class="v4-assertion-item">
          <span class="v4-assertion-chapter">Ch.15</span>
          <span>通道可被人为封印,需特定法器</span>
        </div>
        <div class="v4-assertion-item">
          <span class="v4-assertion-chapter">Ch.22</span>
          <span>存在隐藏通道,连接未知区域</span>
        </div>
      </div>
    </div>
  </div>`;
}

// ─── Panel: Map ─────────────────────────────────────────────────────────────

function panelMap() {
  const places = [
    { name: '鬼门关', x: 40, y: 50 },
    { name: '黄泉路', x: 130, y: 80 },
    { name: '望乡台', x: 220, y: 40 },
    { name: '阎罗殿', x: 300, y: 120 },
    { name: '忘川', x: 100, y: 200 },
    { name: '奈何桥', x: 180, y: 240 },
    { name: '轮回井', x: 320, y: 250 },
  ];

  const routes = [
    [0, 1], [1, 2], [2, 3], [1, 4], [4, 5], [5, 6], [3, 6],
  ];

  const pinEls = places.map(p => `
    <div class="v4-map-pin" style="left:${p.x}px;top:${p.y}px;"></div>
    <div class="v4-map-pin-label" style="left:${p.x + 14}px;top:${p.y - 6}px;">${p.name}</div>`
  ).join('');

  const routeEls = routes.map(([fi, ti]) => {
    const f = places[fi];
    const t = places[ti];
    const dx = t.x - f.x;
    const dy = t.y - f.y;
    const len = Math.sqrt(dx * dx + dy * dy);
    const angle = Math.atan2(dy, dx) * (180 / Math.PI);
    return `<div class="v4-map-route" style="left:${f.x + 5}px;top:${f.y + 5}px;width:${len}px;transform:rotate(${angle}deg);"></div>`;
  }).join('');

  return `
  <div class="v4-panel-map">
    <!-- Map Area -->
    <div class="v4-map-area">
      ${routeEls}
      ${pinEls}
    </div>

    <!-- Inspector -->
    <div class="v4-map-inspector">
      <h4>阎罗殿</h4>
      <div class="v4-map-chapter-range">出现于 Ch.1 — Ch.42</div>
      <div style="font-size:11px;color:var(--v4-muted);">阴界核心区域,审判之所</div>
      <div style="font-size:10px;font-weight:800;color:var(--v4-muted);margin-top:6px;">LINKED FACTS</div>
      <div class="v4-map-linked-facts">
        <div class="v4-map-fact">阴界权力中心,由阎罗王统治</div>
        <div class="v4-map-fact">审判大厅可容纳千名亡魂</div>
        <div class="v4-map-fact">地下设有囚禁特殊灵魂的牢狱</div>
      </div>
    </div>
  </div>`;
}

// ─── Panel: Quality ─────────────────────────────────────────────────────────

function panelQuality() {
  const queues = [
    { name: '待审核', count: 4, selected: true },
    { name: '已冲突', count: 2, selected: false },
    { name: '已过期', count: 1, selected: false },
    { name: '已批准', count: 36, selected: false },
  ];

  const queueRows = queues.map(q => `
    <div class="v4-queue-row${q.selected ? ' selected' : ''}">
      <span class="v4-queue-name">${q.name}</span>
      <span class="v4-queue-count">${q.count}</span>
    </div>`).join('');

  const triages = [
    { name: '角色身份矛盾', severity: 'H' },
    { name: '时间线不一致', severity: 'M' },
    { name: '别名误匹配', severity: 'M' },
    { name: '低置信关系', severity: 'L' },
  ];

  const triageRows = triages.map(t => `
    <div class="v4-triage-row">
      <span class="v4-triage-name">${t.name}</span>
      <span class="v4-severity-badge ${t.severity}">${t.severity}</span>
    </div>`).join('');

  return `
  <div class="v4-panel-quality">
    <!-- Queues -->
    <div class="v4-queue-col">
      <div class="v4-queue-header">队列</div>
      ${queueRows}
    </div>

    <!-- Triage -->
    <div class="v4-triage-col">
      <div class="v4-triage-header">分诊</div>
      ${triageRows}
    </div>

    <!-- Inspector -->
    <div class="v4-quality-inspector">
      <h4>角色身份矛盾</h4>
      <div style="font-size:11px;color:var(--v4-muted);">关联: 白无常, 无常使者</div>
      <div class="v4-error-code">ERR_IDENTITY_CONFLICT
source: Ch.18 — "白无常独自前往阴界"
source: Ch.18 — "无常使者与李青萝同行"
conflict: 同一章节中同一角色出现矛盾状态</div>
      <div class="v4-quality-actions">
        <button class="v4-action-btn primary">合并身份</button>
        <button class="v4-action-btn">标记为别名</button>
        <button class="v4-action-btn">忽略</button>
      </div>
    </div>
  </div>`;
}

// ─── Panel registry ─────────────────────────────────────────────────────────

const PANELS = {
  overview: panelOverview,
  task: panelTask,
  characters: panelCharacters,
  relationships: panelRelationships,
  knowledge: panelKnowledge,
  map: panelMap,
  quality: panelQuality,
};

// ─── Bootstrap ──────────────────────────────────────────────────────────────

let currentDomain = 'overview';

function switchDomain(key) {
  if (!PANELS[key]) return;
  currentDomain = key;

  // Update rail
  document.querySelectorAll('.v4-domain-rail-item').forEach(el => {
    el.classList.toggle('active', el.dataset.domain === key);
  });

  // Render panel
  const content = document.getElementById('panel-content');
  if (content) {
    content.innerHTML = PANELS[key]();
  }
}

function init() {
  // Inject shared CSS
  const style1 = document.createElement('style');
  style1.textContent = SHARED_CSS;
  document.head.appendChild(style1);

  const style2 = document.createElement('style');
  style2.textContent = PANEL_CSS;
  document.head.appendChild(style2);

  // Populate top bar
  const topBar = document.getElementById('top-bar');
  if (topBar) topBar.innerHTML = renderTopBar();

  // Populate rail
  const rail = document.getElementById('rail');
  if (rail) {
    rail.innerHTML = renderRail(currentDomain);
    rail.addEventListener('click', e => {
      const item = e.target.closest('.v4-domain-rail-item');
      if (item && item.dataset.domain) {
        switchDomain(item.dataset.domain);
      }
    });
  }

  // Render default panel
  switchDomain(currentDomain);
}

document.addEventListener('DOMContentLoaded', init);
