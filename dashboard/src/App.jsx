import React, { useState } from 'react';
import { useEngineSocket } from './hooks/useEngineSocket.js';
import Sidebar from './components/Sidebar.jsx';
import TopBar from './components/TopBar.jsx';
import MetricCard from './components/MetricCard.jsx';
import FitnessChart from './components/FitnessChart.jsx';
import PassHeatmap from './components/PassHeatmap.jsx';
import NeatStatus from './components/NeatStatus.jsx';
import IslandPanel from './components/IslandPanel.jsx';
import StrainList from './components/StrainList.jsx';
import EngineLog from './components/EngineLog.jsx';

export default function App() {
  const { state, sendCommand } = useEngineSocket();
  const [activeTab, setActiveTab] = useState('dashboard');

  return (
    <div style={{ display: 'flex', minHeight: '100vh' }}>
      <Sidebar
        connected={state.connected}
        neatReady={state.neatReady}
        activeTab={activeTab}
        setActiveTab={setActiveTab}
      />

      <div style={{ flex: 1, padding: 20, overflow: 'auto' }}>
        <TopBar
          currentModule={state.currentModule}
          goal={state.goal}
          running={state.running}
          generation={state.generation}
        />

        <div style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(4, 1fr)',
          gap: 12,
          marginBottom: 16,
        }}>
          <MetricCard
            label="Best Fitness"
            value={state.bestFitness.toFixed(2)}
            sub={state.fitnessHistory.length > 1 ? `(${state.fitnessHistory[state.fitnessHistory.length - 1]?.fitness.toFixed(2) || '—'})` : ''}
          />
          <MetricCard
            label="Total Generations"
            value={state.generation}
          />
          <MetricCard
            label="NEAT Records"
            value={state.neatRecords}
            sub="/500"
          />
          <MetricCard
            label="Active Strains"
            value={state.strains.length}
          />
        </div>

        <div style={{
          display: 'grid',
          gridTemplateColumns: '1fr 1fr',
          gap: 12,
          marginBottom: 16,
        }}>
          <FitnessChart history={state.fitnessHistory} />
          <PassHeatmap frequencies={state.passFrequency} />
        </div>

        <div style={{
          display: 'grid',
          gridTemplateColumns: '1fr 1fr 1fr',
          gap: 12,
          marginBottom: 16,
        }}>
          <NeatStatus
            records={state.neatRecords}
            confidence={state.neatConfidence}
            ready={state.neatReady}
            species={state.neatSpecies}
          />
          <IslandPanel islands={state.islands} bestPipeline={state.bestPipeline} />
          <StrainList strains={state.strains} />
        </div>

        <EngineLog log={state.log} />
      </div>
    </div>
  );
}