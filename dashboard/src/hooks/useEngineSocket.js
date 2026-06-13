import { useState, useEffect, useRef, useCallback } from 'react';

const MOCK_STATE = {
  connected: false,
  running: false,
  currentModule: 'compute_sum',
  goal: 'minimize_instructions',
  generation: 0,
  bestFitness: -18.0,
  fitnessHistory: [],
  neatRecords: 0,
  neatConfidence: 0.0,
  neatReady: false,
  neatSpecies: 0,
  islands: [
    { id: 0, fitness: -18.0, generation: 0, lead: true },
    { id: 1, fitness: -18.0, generation: 0, lead: false },
    { id: 2, fitness: -18.0, generation: 0, lead: false },
  ],
  strains: [],
  bestPipeline: [],
  passFrequency: {
    constant_folding: 0,
    dead_code: 0,
    cse: 0,
    loop_unroll: 0,
    constant_propagation: 0,
    block_merge: 0,
    strength_reduction: 0,
  },
  log: [],
};

export function useEngineSocket() {
  const [state, setState] = useState(MOCK_STATE);
  const wsRef = useRef(null);
  const reconnectTimer = useRef(null);

  const addLog = useCallback((type, msg) => {
    const ts = new Date().toLocaleTimeString('en-GB');
    setState(s => ({
      ...s,
      log: [{ ts, type, msg }, ...s.log].slice(0, 100),
    }));
  }, []);

  const connect = useCallback(() => {
    try {
      const ws = new WebSocket('ws://localhost:9877/ws');
      wsRef.current = ws;

      ws.onopen = () => {
        setState(s => ({ ...s, connected: true }));
        addLog('good', 'Connected to engine daemon on port 9877');
        ws.send(JSON.stringify({ action: 'subscribe' }));
        ws.send(JSON.stringify({ action: 'ping' }));
      };

      ws.onmessage = (e) => {
        try {
          const msg = JSON.parse(e.data);
          handleMessage(msg);
        } catch {}
      };

      ws.onclose = () => {
        setState(s => ({ ...s, connected: false }));
        addLog('warn', 'Disconnected — retrying in 3s...');
        reconnectTimer.current = setTimeout(connect, 3000);
      };

      ws.onerror = () => {
        addLog('warn', 'WebSocket error — running in demo mode');
        ws.close();
      };
    } catch {
      addLog('warn', 'Cannot connect — running in demo mode');
      startDemoMode();
    }
  }, []);

  const handleMessage = useCallback((msg) => {
    switch (msg.type) {
      case 'fitness_update':
        setState(s => ({
          ...s,
          generation: msg.generation,
          bestFitness: msg.best_fitness,
          currentModule: msg.module_name,
          running: true,
          fitnessHistory: [...s.fitnessHistory, {
            gen: msg.generation,
            fitness: msg.best_fitness
          }].slice(-200),
          islands: msg.islands || s.islands,
          bestPipeline: msg.best_pipeline || s.bestPipeline,
        }));
        break;

      case 'neat_update':
        setState(s => ({
          ...s,
          neatRecords: msg.records,
          neatConfidence: msg.confidence,
          neatReady: msg.ready,
          neatSpecies: msg.species || s.neatSpecies,
        }));
        break;

      case 'strain_update':
        setState(s => ({ ...s, strains: msg.strains }));
        break;

      case 'pass_frequency':
        setState(s => ({ ...s, passFrequency: msg.frequencies }));
        break;

      case 'log':
        addLog(msg.level || 'info', msg.message);
        break;

      case 'pong':
        break;

      default:
        break;
    }
  }, [addLog]);

  // Demo mode — simulates live data when not connected to engine
  const startDemoMode = useCallback(() => {
    addLog('info', 'Demo mode active — connect engine daemon to see live data');
    let gen = 0;
    let fitness = -18.0;
    let neatRec = 0;

    const interval = setInterval(() => {
      gen++;
      neatRec = Math.min(500, neatRec + Math.floor(Math.random() * 3));
      const improvement = (1 - Math.exp(-gen * 0.003)) * 10.8;
      const noise = (Math.random() - 0.5) * 0.3;
      fitness = parseFloat((-18 + improvement + noise).toFixed(2));

      setState(s => ({
        ...s,
        generation: gen,
        bestFitness: fitness,
        running: true,
        currentModule: 'compute_sum',
        goal: 'minimize_instructions',
        neatRecords: neatRec,
        neatConfidence: parseFloat((neatRec / 500).toFixed(2)),
        neatReady: neatRec >= 500,
        neatSpecies: Math.min(12, Math.floor(gen / 80) + 1),
        fitnessHistory: [...s.fitnessHistory, { gen, fitness }].slice(-200),
        islands: [
          { id: 0, fitness: parseFloat((fitness + 0.2).toFixed(2)), generation: gen, lead: true },
          { id: 1, fitness: parseFloat((fitness - 1.9).toFixed(2)), generation: gen - 16, lead: false },
          { id: 2, fitness: parseFloat((fitness - 4.2).toFixed(2)), generation: gen - 38, lead: false },
        ],
        bestPipeline: ['constant_folding', 'dead_code', 'constant_propagation', 'loop_unroll'],
        passFrequency: {
          constant_folding: 0.28,
          dead_code: 0.22,
          cse: 0.18,
          loop_unroll: 0.14,
          constant_propagation: 0.10,
          block_merge: 0.05,
          strength_reduction: 0.03,
        },
        strains: gen > 200 ? [
          { id: 'STR-001', gateLevel: gen > 700 ? 2 : 1, specialty: 'entropy', fitness: parseFloat((fitness + 0.9).toFixed(2)), generations: gen - 150 },
          { id: 'STR-002', gateLevel: gen > 500 ? 1 : 0, specialty: 'evasion', fitness: parseFloat((fitness - 1.2).toFixed(2)), generations: gen - 350 },
          { id: 'STR-003', gateLevel: 0, specialty: 'branch', fitness: parseFloat((fitness - 4.1).toFixed(2)), generations: gen - 500 },
        ] : [],
      }));

      if (gen % 50 === 0) {
        addLog('good', `New best fitness: ${fitness.toFixed(2)} at generation ${gen}`);
      }
      if (neatRec === 500) {
        addLog('accent', 'NEAT brain ready — NM confidence threshold reached');
      }
    }, 800);

    return () => clearInterval(interval);
  }, [addLog]);

  useEffect(() => {
    connect();
    return () => {
      wsRef.current?.close();
      clearTimeout(reconnectTimer.current);
    };
  }, []);

  const sendCommand = useCallback((action, params = {}) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ action, ...params }));
    }
  }, []);

  return { state, sendCommand };
}