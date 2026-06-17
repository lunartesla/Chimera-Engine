import { useState, useEffect, useRef, useCallback } from 'react';

// ── Real wire protocol only ───────────────────────────────────────────────
// The engine broadcasts exactly 3 message types over ws://localhost:9877/ws:
//   fitness_update { module_name, generation, best_fitness, best_pipeline, islands }
//   neat_update    { records, confidence, ready, species }
//   log            { level, message }
// Note: `islands` is a static formula on the Rust side (best_fitness ± offset),
// not real per-island state. We display it as-is but don't pretend it's more.
// Strains / pass-frequency / blueprints have NO backing broadcast yet — the UI
// must say so honestly rather than fabricate data.

const MAX_HISTORY = 200;
const MAX_LOG = 100;

const INITIAL_STATE = {
  connected: false,
  mode: 'connecting',        // 'connecting' | 'live' | 'demo'
  currentModule: null,
  generation: 0,
  bestFitness: null,
  fitnessHistory: [],
  bestPipeline: [],
  islands: [],
  neatRecords: 0,
  neatConfidence: 0,
  neatReady: false,
  neatSpecies: 0,
  log: [],
};

function nowTs() {
  return new Date().toLocaleTimeString('en-GB');
}

export function useEngineSocket() {
  const [state, setState] = useState(INITIAL_STATE);
  const wsRef = useRef(null);
  const demoTimerRef = useRef(null);
  const reconnectRef = useRef(null);
  const mountedRef = useRef(true);

  // EngineLog component expects { ts, type, msg } — match it exactly
  const addLog = useCallback((type, msg) => {
    setState(s => ({
      ...s,
      log: [{ ts: nowTs(), type, msg }, ...s.log].slice(0, MAX_LOG),
    }));
  }, []);

  const stopDemo = useCallback(() => {
    if (demoTimerRef.current) {
      clearInterval(demoTimerRef.current);
      demoTimerRef.current = null;
    }
  }, []);

  // ── apply a real fitness_update message ───────────────────────────────
  const applyFitnessUpdate = useCallback((msg) => {
    setState(s => ({
      ...s,
      currentModule: msg.module_name ?? s.currentModule,
      generation: msg.generation,
      bestFitness: msg.best_fitness,
      bestPipeline: msg.best_pipeline ?? s.bestPipeline,
      islands: msg.islands ?? s.islands,
      fitnessHistory: [...s.fitnessHistory, { gen: msg.generation, fitness: msg.best_fitness }].slice(-MAX_HISTORY),
    }));
  }, []);

  const applyNeatUpdate = useCallback((msg) => {
    setState(s => ({
      ...s,
      neatRecords: msg.records,
      neatConfidence: msg.confidence,
      neatReady: msg.ready,
      neatSpecies: msg.species,
    }));
  }, []);

  const handleMessage = useCallback((raw) => {
    let msg;
    try { msg = JSON.parse(raw); } catch { return; }
    switch (msg.type) {
      case 'fitness_update': applyFitnessUpdate(msg); break;
      case 'neat_update':    applyNeatUpdate(msg); break;
      case 'log':            addLog(msg.level || 'info', msg.message); break;
      default: break; // unknown type — ignore, don't fabricate
    }
  }, [applyFitnessUpdate, applyNeatUpdate, addLog]);

  // ── demo mode: simulates the SAME 3 message shapes, nothing extra ──────
  const startDemo = useCallback(() => {
    if (demoTimerRef.current) return; // already running
    setState(s => ({ ...s, mode: 'demo' }));
    addLog('warn', 'Demo mode — no engine connection, showing simulated data');

    let gen = 0;
    let neatRec = 0;

    demoTimerRef.current = setInterval(() => {
      gen++;
      neatRec = Math.min(500, neatRec + Math.floor(Math.random() * 3));
      const improvement = (1 - Math.exp(-gen * 0.003)) * 10.8;
      const fitness = parseFloat((-18 + improvement + (Math.random() - 0.5) * 0.3).toFixed(2));

      applyFitnessUpdate({
        module_name: 'compute_sum',
        generation: gen,
        best_fitness: fitness,
        best_pipeline: ['constant_folding', 'dead_code', 'constant_propagation', 'loop_unroll'],
        islands: [
          { id: 0, fitness: parseFloat((fitness + 0.2).toFixed(2)), generation: gen, lead: true },
          { id: 1, fitness: parseFloat((fitness - 1.9).toFixed(2)), generation: Math.max(0, gen - 16), lead: false },
          { id: 2, fitness: parseFloat((fitness - 4.2).toFixed(2)), generation: Math.max(0, gen - 38), lead: false },
        ],
      });
      applyNeatUpdate({
        records: neatRec,
        confidence: parseFloat((neatRec / 500).toFixed(2)),
        ready: neatRec >= 500,
        species: Math.min(12, Math.floor(gen / 80) + 1),
      });

      if (gen % 50 === 0) addLog('good', `New best fitness: ${fitness.toFixed(2)} at generation ${gen}`);
    }, 800);
  }, [addLog, applyFitnessUpdate, applyNeatUpdate]);

  // ── real WebSocket connection ────────────────────────────────────────
  const connect = useCallback(() => {
    clearTimeout(reconnectRef.current);
    let ws;
    try {
      ws = new WebSocket('ws://localhost:9877/ws');
    } catch {
      startDemo();
      return;
    }
    wsRef.current = ws;

    ws.onopen = () => {
      if (!mountedRef.current) return;
      stopDemo();
      setState(s => ({ ...s, connected: true, mode: 'live' }));
      addLog('good', 'Connected to engine daemon on port 9877');
    };

    ws.onmessage = (e) => handleMessage(e.data);

    ws.onclose = () => {
      if (!mountedRef.current) return;
      setState(s => ({ ...s, connected: false }));
      startDemo();
      reconnectRef.current = setTimeout(connect, 5000);
    };

    ws.onerror = () => { /* onclose follows; nothing to do here */ };
  }, [addLog, handleMessage, startDemo, stopDemo]);

  useEffect(() => {
    mountedRef.current = true;
    connect();
    return () => {
      mountedRef.current = false;
      wsRef.current?.close();
      clearTimeout(reconnectRef.current);
      stopDemo();
    };
  }, [connect, stopDemo]);

  const sendCommand = useCallback((action, params = {}) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ action, ...params }));
    }
  }, []);

  return { state, sendCommand };
}
