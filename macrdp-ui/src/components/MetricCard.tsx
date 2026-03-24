interface MetricCardProps {
  value: string | number;
  label: string;
  unit?: string;
}

function MetricCard({ value, label, unit }: MetricCardProps) {
  return (
    <div className="flex flex-col items-center justify-center rounded-lg bg-black/[0.04] px-4 py-5">
      <div className="text-2xl font-semibold text-macos-text">
        {value}
        {unit && (
          <span className="ml-1 text-sm font-normal text-macos-secondary">
            {unit}
          </span>
        )}
      </div>
      <div className="mt-1 text-xs text-macos-secondary">{label}</div>
    </div>
  );
}

export default MetricCard;
