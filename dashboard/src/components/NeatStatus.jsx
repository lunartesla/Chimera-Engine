import React from 'react';

const TERRACOTTA = '#DA7756';
const GREEN = '#81C784';
const WARNING = '#F6A623';

function ProgressBar({ label, value, max, color }) {
  const pct = Math.min(100, (value / max) * 100);
  return (
    <div style={{ marginBottom: 8 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 10, marginBottom: 2 }}>
        <span style={{ color: 'rgba(250,249,245,0.4)' }}>{label}</span>
        <span style={{ color: '#FAF9F5' }}>{value}/{max}</span>
      </div>
      <div style={{ height: 4, background: 'rgba(250,249,245,0.05)', borderRadius: 2 }}>
        <div style={{ width: `${pct}%`, height: '100%', background: color, borderRadius: 2 }} />
      </div>
    </div>
  );
}

export default function NeatStatus({ records, confidence, ready, species }) {
  return (
    <div style={{
      background: '#252422',
      border: '1px solid rgba(250,249,245,0.07)',
      borderRadius: 6,
      padding: 12,
    }}>
      <div style={{ color: 'rgba(250,249,245,0.4)', fontSize: 11, marginBottom: 8 }}>
        NEAT Brain Status
      </div>

      {!ready && (
        <div style={{
          background: 'rgba(246,166,35,0.15)',
          border: '1px solid rgba(246,166,35,0.3)',
          borderRadius: 4,
          padding: '6px 8px',
          marginBottom: 8,
          fontSize: 11,
          color: WARNING,
        }}>
          ⚠ Warming up — {500 - records} records until ready
        </div>
      )}

      {ready && (
        <div style={{
          background: 'rgba(129,199,132,0.15)',
          border: '1px solid rgba(129,199,132,0.3)',
          borderRadius: 4,
          padding: '6px 8px',
          marginBottom: 8,
          fontSize: 11,
          color: GREEN,
        }}>
          ✓ NEAT brain active
        </div>
      )}

      <ProgressBar label="Confidence" value={Math.round(confidence * 100)} max={100} color={TERRACOTTA} />
      <ProgressBar label="Records" value={records} max={500} color={TERRACOTTA} />
      <ProgressBar label="Species" value={species} max={12} color={TERRACOTTA} />
    </div>
  );
}