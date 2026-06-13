import React from 'react';

const TERRACOTTA = '#DA7756';

export default function MetricCard({ label, value, sub }) {
  return (
    <div style={{
      background: '#252422',
      border: '1px solid rgba(250,249,245,0.07)',
      borderRadius: 6,
      padding: '12px 16px',
    }}>
      <div style={{ color: 'rgba(250,249,245,0.4)', fontSize: 11, marginBottom: 4 }}>
        {label}
      </div>
      <div style={{ fontSize: 20, fontWeight: 600, color: '#FAF9F5' }}>
        {value}
        {sub && (
          <span style={{ color: TERRACOTTA, fontSize: 12, marginLeft: 4 }}>{sub}</span>
        )}
      </div>
    </div>
  );
}