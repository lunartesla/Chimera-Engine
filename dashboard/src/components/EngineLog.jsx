import React from 'react';

const TERRACOTTA = '#DA7756';
const GREEN = '#81C784';
const WARNING = '#F6A623';

function getLogColor(type) {
  switch (type) {
    case 'good': return GREEN;
    case 'accent': return TERRACOTTA;
    case 'warn': return WARNING;
    default: return 'rgba(250,249,245,0.4)';
  }
}

export default function EngineLog({ log }) {
  return (
    <div style={{
      background: '#252422',
      border: '1px solid rgba(250,249,245,0.07)',
      borderRadius: 6,
      padding: 12,
    }}>
      <div style={{ color: 'rgba(250,249,245,0.4)', fontSize: 11, marginBottom: 8 }}>
        Engine Log
      </div>
      <div style={{
        maxHeight: 160,
        overflow: 'auto',
        fontFamily: 'ui-monospace, Consolas, monospace',
        fontSize: 11,
      }}>
        {log.map((entry, i) => (
          <div key={i} style={{ display: 'flex', gap: 6, marginBottom: 2 }}>
            <span style={{ color: 'rgba(250,249,245,0.3)' }}>{entry.ts}</span>
            <span style={{ color: getLogColor(entry.type), fontWeight: 500 }}>▸</span>
            <span style={{ color: getLogColor(entry.type) }}>{entry.msg}</span>
          </div>
        ))}
      </div>
    </div>
  );
}