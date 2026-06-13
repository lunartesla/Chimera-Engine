import React from 'react';

const TERRACOTTA = '#DA7756';

export default function BestPipeline({ pipeline }) {
  if (!pipeline || pipeline.length === 0) return null;

  return (
    <div style={{ marginTop: 8 }}>
      <div style={{ color: 'rgba(250,249,245,0.4)', fontSize: 10, marginBottom: 4 }}>
        Best Pipeline
      </div>
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
        {pipeline.map((pass, i) => (
          <span key={i} style={{
            fontSize: 10,
            padding: '2px 6px',
            background: 'rgba(218,119,86,0.15)',
            border: '1px solid rgba(218,119,86,0.3)',
            borderRadius: 3,
            color: TERRACOTTA,
          }}>
            {pass.replace(/_/g, ' ')}
          </span>
        ))}
      </div>
    </div>
  );
}