import React from 'react';

const PASS_LABELS = {
  constant_folding: 'const_fold',
  dead_code: 'dead_code',
  cse: 'cse',
  loop_unroll: 'loop_unroll',
  constant_propagation: 'const_prop',
  block_merge: 'block_merge',
  strength_reduction: 'strength_red',
};

const TERRACOTTA = '#DA7756';

export default function PassHeatmap({ frequencies }) {
  const passes = Object.entries(frequencies);

  return (
    <div style={{
      background: '#252422',
      border: '1px solid rgba(250,249,245,0.07)',
      borderRadius: 6,
      padding: 12,
    }}>
      <div style={{ color: 'rgba(250,249,245,0.4)', fontSize: 11, marginBottom: 8 }}>
        Pass Frequency
      </div>
      <div style={{ display: 'flex', alignItems: 'flex-end', height: 100, gap: 8 }}>
        {passes.map(([pass, freq]) => (
          <div key={pass} style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
            <div style={{
              width: '100%',
              height: `${freq * 100}%`,
              background: `rgba(218,119,86,${0.15 + freq * 0.7})`,
              borderRadius: 2,
              minHeight: 4,
            }} />
          </div>
        ))}
      </div>
      <div style={{ display: 'flex', gap: 8, marginTop: 6 }}>
        {passes.map(([pass]) => (
          <div key={pass} style={{ flex: 1, fontSize: 9, color: 'rgba(250,249,245,0.4)', textAlign: 'center' }}>
            {PASS_LABELS[pass] || pass}
          </div>
        ))}
      </div>
    </div>
  );
}