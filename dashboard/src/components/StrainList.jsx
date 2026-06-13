import React from 'react';

const TERRACOTTA = '#DA7756';
const GREEN = '#81C784';

function getGateStyle(level) {
  switch (level) {
    case 0: return { bg: 'rgba(250,249,245,0.1)', color: '#FAF9F5' };
    case 1: return { bg: 'rgba(218,119,86,0.15)', color: '#FAF9F5' };
    case 2: return { bg: 'rgba(218,119,86,0.25)', color: '#FAF9F5' };
    case 3: return { bg: 'rgba(218,119,86,0.4)', color: '#FAF9F5' };
    default: return { bg: 'rgba(218,119,86,0.15)', color: '#FAF9F5' };
  }
}

function getGateLabel(level) {
  switch (level) {
    case 0: return 'None';
    case 1: return 'Gate 1';
    case 2: return 'Gate 2';
    case 3: return 'Gate 3';
    default: return 'None';
  }
}

export default function StrainList({ strains }) {
  if (!strains || strains.length === 0) return null;

  return (
    <div style={{
      background: '#252422',
      border: '1px solid rgba(250,249,245,0.07)',
      borderRadius: 6,
      padding: 12,
    }}>
      <div style={{ color: 'rgba(250,249,245,0.4)', fontSize: 11, marginBottom: 8 }}>
        Active Strains
      </div>
      {strains.map(strain => {
        const gateStyle = getGateStyle(strain.gateLevel);
        return (
          <div key={strain.id} style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: '6px 0',
            borderBottom: '1px solid rgba(250,249,245,0.03)',
            fontSize: 11,
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <span style={{ color: TERRACOTTA, fontWeight: 500 }}>{strain.id}</span>
              <span style={{
                padding: '1px 6px',
                background: gateStyle.bg,
                borderRadius: 3,
                fontSize: 10,
              }}>
                {getGateLabel(strain.gateLevel)}
              </span>
              <span style={{ color: 'rgba(250,249,245,0.4)' }}>{strain.specialty}</span>
            </div>
            <div style={{ textAlign: 'right' }}>
              <div style={{ color: GREEN, fontWeight: 500 }}>{strain.fitness.toFixed(2)}</div>
              <div style={{ color: 'rgba(250,249,245,0.3)', fontSize: 10 }}>
                {strain.generations} gens
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}