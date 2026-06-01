export function invoiceTotalCents(lines: number[]): number {
  return lines.reduce((total, line) => total + line, 0);
}
