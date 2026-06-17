import React from 'react';

const TERRACOTTA = '#DA7756';
const GREEN = '#81C784';

export default function TopBar({ currentModule, running, generation }) {
  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      padding: '12px 20px',
      background: 'rgba(37,36,34,0.5)',
      borderBottom: '1px solid rgba(250,249,245,0.05)',
      marginBottom: 16,
    }}>
      <div>
        <h1 style={{ fontSize: 18, fontWeight: 600, margin: 0, color: '#FAF9F5' }}>
          Chimera Engine Dashboard
        </h1>
        <div style={{ fontSize: 11, color: 'rgba(250,249,245,0.4)', marginTop: 2 }}>
          Module: <span style={{ color: TERRACOTTA }}>{currentModule ?? '—'}</span>
        </div>
      </div>

      <div style={{ display: 'flex', gap: 12 }}>
        <span style={{
          padding: '4px 12px',
          borderRadius: 12,
          fontSize: 11,
          fontWeight: 500,
          background: running ? 'rgba(129,199,132,0.15)' : 'rgba(250,249,245,0.05)',
          color: running ? GREEN : 'rgba(250,249,245,0.4)',
        }}>
          {running ? 'Running' : 'Idle'}
        </span>
        <span style={{
          padding: '4px 12px',
          borderRadius: 12,
          fontSize: 11,
          fontWeight: 500,
          background: 'rgba(218,119,86,0.15)',
          color: TERRACOTTA,
        }}>
          Gen {generation}
        </span>
      </div>
    </div>
  );
}