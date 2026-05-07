interface FocusStatusProps {
  label: string;
}

export function FocusStatus({ label }: FocusStatusProps) {
  return <span>{label}</span>;
}
