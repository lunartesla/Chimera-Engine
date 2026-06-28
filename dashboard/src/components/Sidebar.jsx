import React from 'react';
import { LayoutDashboard, Cpu, GitBranch, SlidersHorizontal } from 'lucide-react';

const TERRACOTTA = '#DA7756';
const GREEN = '#81C784';
const WARNING = '#F6A623';

// Only tabs with REAL backing data from the engine's broadcast protocol.
// Strains and Tuning now have real broadcasts (strain_update / tuning_update)
// — see useEngineSocket.js. Blueprints / Passes are still removed since the
// engine doesn't broadcast those yet; add them back once it does.
const NAV_ITEMS = [
  { id: 'dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { id: 'neat', label: 'NEAT Brain', icon: Cpu },
  { id: 'strains', label: 'Strains', icon: GitBranch },
  { id: 'tuning', label: 'Tuning', icon: SlidersHorizontal },
];

export default function Sidebar({ connected, neatReady, activeTab, setActiveTab }) {
  return (
    <div style={{
      width: 200,
      background: '#1F1E1D',
      height: '100vh',
      padding: '16px 12px',
      display: 'flex',
      flexDirection: 'column',
      gap: 4,
    }}>
      <div style={{ marginBottom: 24 }}>
        <div style={{ fontSize: 14, fontWeight: 600, color: TERRACOTTA, marginBottom: 2 }}>
          Chimera
        </div>
        <div style={{ fontSize: 11, color: 'rgba(250,249,245,0.4)' }}>
          Evolutionary Engine
        </div>
      </div>

      {NAV_ITEMS.map(item => {
        const Icon = item.icon;
        const isActive = activeTab === item.id;
        return (
          <div
            key={item.id}
            onClick={() => setActiveTab(item.id)}
            style={{
              display: 'flex', alignItems: 'center', gap: 8,
              padding: '8px 12px', borderRadius: 6, cursor: 'pointer',
              background: isActive ? 'rgba(218,119,86,0.15)' : 'transparent',
              color: isActive ? TERRACOTTA : '#FAF9F5',
            }}
          >
            <Icon size={16} />
            <span style={{ fontSize: 12 }}>{item.label}</span>
            {item.id === 'neat' && !neatReady && (
              <span style={{ width: 6, height: 6, borderRadius: '50%', background: WARNING, marginLeft: 'auto' }} />
            )}
          </div>
        );
      })}

      <div style={{ marginTop: 'auto', padding: '12px 0', fontSize: 11 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, color: 'rgba(250,249,245,0.4)' }}>
          <span style={{ width: 8, height: 8, borderRadius: '50%', background: connected ? GREEN : WARNING }} />
          <span>Daemon {connected ? 'connected' : 'disconnected'}</span>
        </div>
      </div>
    </div>
  );
}
