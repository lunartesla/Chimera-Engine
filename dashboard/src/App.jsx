import React, { useState } from 'react';
import { useEngineSocket } from './hooks/useEngineSocket.js';
import Sidebar from './components/Sidebar.jsx';
import TopBar from './components/TopBar.jsx';
import MetricCard from './components/MetricCard.jsx';
import FitnessChart from './components/FitnessChart.jsx';
import NeatStatus from './components/NeatStatus.jsx';
import IslandPanel from './components/IslandPanel.jsx';
import EngineLog from './components/EngineLog.jsx';
import StrainList from './components/StrainList.jsx';

const DIM = 'rgba(250,249,245,0.4)';
const C = '#252422';
const B = '1px solid rgba(250,249,245,0.07)';

function ModeBadge({ mode }) {
  const cfg = {
    live:       { label: 'Live',    bg: 'rgba(129,199,132,0.15)', fg: '#81C784' },
    demo:       { label: 'Demo',    bg: 'rgba(246,166,35,0.15)',  fg: '#F6A623' },
    connecting: { label: 'Connecting…', bg: 'rgba(250,249,245,0.05)', fg: DIM },
  }[mode] ?? { label: mode, bg: 'rgba(250,249,245,0.05)', fg: DIM };

  return (
    <span style={{ padding: '4px 10px', borderRadius: 12, fontSize: 11, fontWeight: 500, background: cfg.bg, color: cfg.fg }}>
      {cfg.label}
    </span>
  );
}

function DashboardTab({ state }) {
  return (
    <>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 12, marginBottom: 16 }}>
        <MetricCard label="Best Fitness" value={state.bestFitness !== null ? state.bestFitness.toFixed(2) : '—'} />
        <MetricCard label="Generation" value={state.generation} />
        <MetricCard label="NEAT Records" value={state.neatRecords} sub="/500" />
        <MetricCard label="Species" value={state.neatSpecies} />
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: 12, marginBottom: 16 }}>
        <FitnessChart history={state.fitnessHistory} />
        <NeatStatus records={state.neatRecords} confidence={state.neatConfidence} ready={state.neatReady} species={state.neatSpecies} />
      </div>
      {state.islands.length > 0 && (
        <div style={{ marginBottom: 16 }}>
          <IslandPanel islands={state.islands} bestPipeline={state.bestPipeline} />
        </div>
      )}
      <EngineLog log={state.log} />
    </>
  );
}

function NeatTab({ state }) {
  const pct = Math.round(state.neatConfidence * 100);
  return (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
      <NeatStatus records={state.neatRecords} confidence={state.neatConfidence} ready={state.neatReady} species={state.neatSpecies} />
      <div style={{ background: C, border: B, borderRadius: 6, padding: 16 }}>
        <div style={{ color: DIM, fontSize: 11, marginBottom: 12 }}>Brain Detail</div>
        {[
          { label: 'Records collected', val: `${state.neatRecords} / 500` },
          { label: 'Confidence', val: `${pct}%` },
          { label: 'Species active', val: state.neatSpecies },
          { label: 'Status', val: state.neatReady ? 'Active' : 'Warming up' },
        ].map(({ label, val }) => (
          <div key={label} style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0', borderBottom: B, fontSize: 12 }}>
            <span style={{ color: DIM }}>{label}</span>
            <span style={{ color: '#FAF9F5' }}>{val}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function StrainsTab({ state }) {
  const GATE_LABELS = ['Stuck-tracking', 'Training', 'Confident', 'Stabilizing → promotion'];
  return (
    <div>
      {(!state.strains || state.strains.length === 0) ? (
        <div style={{ background: C, border: B, borderRadius: 6, padding: 16, color: DIM, fontSize: 12 }}>
          No active strains — one forks automatically when a module stops improving for a
          few cycles in a row (tunable in the Tuning tab).
        </div>
      ) : (
        <>
          <StrainList strains={state.strains} />
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 12, marginTop: 12 }}>
            {state.strains.map(s => (
              <div key={s.id} style={{ background: C, border: B, borderRadius: 6, padding: 14 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 8 }}>
                  <span style={{ color: '#DA7756', fontWeight: 500, fontSize: 12 }}>{s.id}</span>
                  <span style={{ color: DIM, fontSize: 11 }}>{GATE_LABELS[s.gateLevel] ?? 'Unknown'}</span>
                </div>
                {[
                  { label: 'Specialty (module)', val: s.specialty },
                  { label: 'Task class', val: s.taskClass },
                  { label: 'Fitness', val: s.fitness?.toFixed?.(2) ?? s.fitness },
                  { label: 'Generations run', val: s.generations },
                  { label: 'NM records', val: `${s.nmRecords}` },
                  { label: 'NM confidence', val: `${Math.round((s.nmConfidence ?? 0) * 100)}%` },
                  { label: 'Forked at', val: s.forkTimestamp },
                ].map(({ label, val }) => (
                  <div key={label} style={{ display: 'flex', justifyContent: 'space-between', padding: '4px 0', fontSize: 11 }}>
                    <span style={{ color: DIM }}>{label}</span>
                    <span style={{ color: '#FAF9F5' }}>{val}</span>
                  </div>
                ))}
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

function TuningSlider({ label, hint, value, min, max, step = 1, onChange }) {
  return (
    <div style={{ marginBottom: 18 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 4 }}>
        <span style={{ fontSize: 12, color: '#FAF9F5' }}>{label}</span>
        <span style={{ fontSize: 12, color: '#DA7756', fontWeight: 500 }}>{value}</span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{ width: '100%' }}
      />
      <div style={{ fontSize: 10, color: DIM, marginTop: 2 }}>{hint}</div>
    </div>
  );
}

function TuningTab({ state, sendCommand }) {
  const t = state.tuning;
  if (!t) {
    return (
      <div style={{ background: C, border: B, borderRadius: 6, padding: 16, color: DIM, fontSize: 12 }}>
        Waiting for the daemon to report current tuning values…
      </div>
    );
  }
  const set = (field, value) => sendCommand('set_tuning', { [field]: value });
  return (
    <div style={{ background: C, border: B, borderRadius: 6, padding: 20, maxWidth: 480 }}>
      <div style={{ color: DIM, fontSize: 11, marginBottom: 16 }}>
        Live daemon settings — changes apply on the next cycle, no restart needed.
      </div>
      <TuningSlider
        label="Stuck cycles before fork"
        hint="How many cycles a module can plateau before a strain forks to explore it independently."
        value={t.stuck_cycles_before_fork} min={1} max={20}
        onChange={(v) => set('stuck_cycles_before_fork', v)}
      />
      <TuningSlider
        label="Generations per evolve batch"
        hint="Generations run per cycle, for both the origin population and each strain's background burst."
        value={t.evolve_batch_size} min={10} max={1000} step={10}
        onChange={(v) => set('evolve_batch_size', v)}
      />
      <TuningSlider
        label="NEAT ready threshold"
        hint="Accepted-mutation records the brain needs before it's trusted for predictions."
        value={t.nm_ready_threshold} min={50} max={5000} step={50}
        onChange={(v) => set('nm_ready_threshold', v)}
      />
      <TuningSlider
        label="Strain generation cap"
        hint="Safety cap — a strain that never gets promoted stops after this many generations."
        value={t.strain_generation_cap} min={500} max={20000} step={500}
        onChange={(v) => set('strain_generation_cap', v)}
      />
    </div>
  );
}

export default function App() {
  const { state, sendCommand } = useEngineSocket();
  const [activeTab, setActiveTab] = useState('dashboard');

  return (
    <div style={{ display: 'flex', minHeight: '100vh' }}>
      <Sidebar connected={state.connected} neatReady={state.neatReady} activeTab={activeTab} setActiveTab={setActiveTab} />
      <div style={{ flex: 1, padding: 20, overflow: 'auto' }}>
        <div style={{ display: 'flex', justifyContent: 'flex-end', marginBottom: -8 }}>
          <ModeBadge mode={state.mode} />
        </div>
        <TopBar currentModule={state.currentModule} running={state.connected || state.mode === 'demo'} generation={state.generation} />
        {activeTab === 'neat' && <NeatTab state={state} />}
        {activeTab === 'strains' && <StrainsTab state={state} />}
        {activeTab === 'tuning' && <TuningTab state={state} sendCommand={sendCommand} />}
        {activeTab === 'dashboard' && <DashboardTab state={state} />}
      </div>
    </div>
  );
}
