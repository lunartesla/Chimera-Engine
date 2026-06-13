import React from 'react';
import BestPipeline from './BestPipeline.jsx';

const TERRACOTTA = '#DA7756';
const GREEN = '#81C784';

export default function IslandPanel({ islands, bestPipeline }) {
  if (!islands) return null;

  return (
    <div>
      <div style={{ color: 'rgba(250,249,245,0.4)', fontSize: 11, marginBottom: 8 }}>
        Islands
      </div>
      <div style={{ display: 'flex', gap: 8, marginBottom: 8 }}>
        {islands.map(island => (
          <div key={island.id} style={{
            flex: 1,
            background: '#252422',
            border: '1px solid rgba(250,249,245,0.07)',
            borderRadius: 6,
            padding: 10,
            position: 'relative',
          }}>
            {island.lead && (
              <span style={{
                position: 'absolute',
                top: 6,
                right: 6,
                fontSize: 9,
                padding: '1px 4px',
                background: GREEN,
                color: '#1F1E1D',
                borderRadius: 3,
                fontWeight: 600,
              }}>
                LEAD
              </span>
            )}
            <div style={{ fontSize: 11, color: 'rgba(250,249,245,0.4)', marginBottom: 4 }}>
              Island {island.id}
            </div>
            <div style={{ fontSize: 16, fontWeight: 600, color: island.lead ? TERRACOTTA : '#FAF9F5', marginBottom: 2 }}>
              {island.fitness.toFixed(2)}
            </div>
            <div style={{ fontSize: 10, color: 'rgba(250,249,245,0.3)' }}>
              gen {island.generation}
            </div>
          </div>
        ))}
      </div>
      <BestPipeline pipeline={bestPipeline} />
    </div>
  );
}