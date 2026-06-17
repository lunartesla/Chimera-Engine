import React, { useState } from 'react';
import { useEngineSocket } from './hooks/useEngineSocket.js';
import Sidebar from './components/Sidebar.jsx';
import TopBar from './components/TopBar.jsx';
import MetricCard from './components/MetricCard.jsx';
import FitnessChart from './components/FitnessChart.jsx';
import NeatStatus from './components/NeatStatus.jsx';
import IslandPanel from './components/IslandPanel.jsx';
import EngineLog from './components/EngineLog.jsx';

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

export default function App() {
  const { state } = useEngineSocket();
  const [activeTab, setActiveTab] = useState('dashboard');

  return (
    <div style={{ display: 'flex', minHeight: '100vh' }}>
      <Sidebar connected={state.connected} neatReady={state.neatReady} activeTab={activeTab} setActiveTab={setActiveTab} />
      <div style={{ flex: 1, padding: 20, overflow: 'auto' }}>
        <div style={{ display: 'flex', justifyContent: 'flex-end', marginBottom: -8 }}>
          <ModeBadge mode={state.mode} />
        </div>
        <TopBar currentModule={state.currentModule} running={state.connected || state.mode === 'demo'} generation={state.generation} />
        {activeTab === 'neat' ? <NeatTab state={state} /> : <DashboardTab state={state} />}
      </div>
    </div>
  );
}
