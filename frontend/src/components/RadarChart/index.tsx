import { memo, useMemo } from 'react';

export interface RadarDataPoint {
  label: string;
  value: number;
}

export interface RadarChartProps {
  data: RadarDataPoint[];
  size?: number;
  className?: string;
}

const getPoint = (center: number, radius: number, angle: number) => ({
  x: center + radius * Math.cos(angle),
  y: center + radius * Math.sin(angle),
});

const toPointsStr = (pts: { x: number; y: number }[]) =>
  pts.map((p) => `${p.x.toFixed(2)},${p.y.toFixed(2)}`).join(' ');

export const RadarChart = memo(({ data, size = 120, className = '' }: RadarChartProps) => {
  const center = size / 2;
  const maxRadius = size / 2 - 18;
  const n = data.length;

  const geometry = useMemo(() => {
    if (n === 0) return null;

    const angleStep = (Math.PI * 2) / n;
    const vertexAngles = Array.from({ length: n }, (_, i) => -Math.PI / 2 + i * angleStep);

    const levels = [1, 2 / 3, 1 / 3];
    const bgPolygons = levels.map((level) =>
      toPointsStr(vertexAngles.map((a) => getPoint(center, maxRadius * level, a))),
    );

    const axes = vertexAngles.map((a) => {
      const p = getPoint(center, maxRadius, a);
      return { x1: center, y1: center, x2: p.x, y2: p.y };
    });

    const dataPts = data.map((d, i) => {
      const clamped = Math.min(Math.max(d.value, 0), 1);
      return getPoint(center, clamped * maxRadius, vertexAngles[i]);
    });

    const labelPositions = vertexAngles.map((a) => getPoint(center, maxRadius + 10, a));

    return {
      bgPolygons,
      axes,
      dataPts,
      dataPolygon: toPointsStr(dataPts),
      labelPositions,
    };
  }, [data, n, center, maxRadius]);

  if (!geometry || n === 0) {
    return <svg width={size} height={size} className={className} />;
  }

  return (
    <svg width={size} height={size} className={className} viewBox={`0 0 ${size} ${size}`}>
      {geometry.bgPolygons.map((pts, i) => (
        <polygon
          key={`bg-${i}`}
          points={pts}
          fill="none"
          strokeWidth={1}
          className="stroke-line-default"
        />
      ))}
      {geometry.axes.map((axis, i) => (
        <line
          key={`axis-${i}`}
          x1={axis.x1}
          y1={axis.y1}
          x2={axis.x2}
          y2={axis.y2}
          strokeWidth={1}
          className="stroke-line-default"
        />
      ))}
      <polygon
        points={geometry.dataPolygon}
        fillOpacity={0.15}
        strokeWidth={1.5}
        strokeLinejoin="round"
        className="fill-ai stroke-ai"
      />
      {geometry.dataPts.map((p, i) => (
        <circle key={`pt-${i}`} cx={p.x} cy={p.y} r={2} className="fill-ai" />
      ))}
      {geometry.labelPositions.map((p, i) => (
        <text
          key={`label-${i}`}
          x={p.x}
          y={p.y}
          textAnchor="middle"
          dominantBaseline="middle"
          className="text-2xs fill-on-surface-tertiary"
        >
          {data[i].label}
        </text>
      ))}
    </svg>
  );
});
