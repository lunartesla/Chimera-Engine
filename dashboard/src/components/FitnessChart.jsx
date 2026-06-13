import React from 'react';
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';

const TERRACOTTA = '#DA7756';

export default function FitnessChart({ history }) {
  return (
    <div style={{
      background: '#252422',
      border: '1px solid rgba(250,249,245,0.07)',
      borderRadius: 6,
      padding: 12,
    }}>
      <div style={{ color: 'rgba(250,249,245,0.4)', fontSize: 11, marginBottom: 8 }}>
        Fitness History
      </div>
      <ResponsiveContainer width="100%" height={180}>
        <LineChart data={history}>
          <defs>
            <linearGradient id="fitnessFill" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={TERRACOTTA} stopOpacity={0.08} />
              <stop offset="100%" stopColor={TERRACOTTA} stopOpacity={0} />
            </linearGradient>
          </defs>
          <XAxis
            dataKey="gen"
            tick={{ fontSize: 10, fill: 'rgba(250,249,245,0.3)' }}
            axisLine={false}
            tickLine={false}
          />
          <YAxis
            tick={{ fontSize: 10, fill: 'rgba(250,249,245,0.3)' }}
            axisLine={false}
            tickLine={false}
            width={30}
          />
          <Tooltip
            contentStyle={{
              background: '#252422',
              border: '1px solid rgba(250,249,245,0.07)',
              fontSize: 11,
            }}
            labelStyle={{ color: '#FAF9F5' }}
          />
          <Line
            type="monotone"
            dataKey="fitness"
            stroke={TERRACOTTA}
            strokeWidth={2}
            dot={false}
            fillOpacity={1}
            fill="url(#fitnessFill)"
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}