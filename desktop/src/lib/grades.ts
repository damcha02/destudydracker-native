export const swissGrades = [4.0, 4.25, 4.5, 4.75, 5.0, 5.25, 5.5, 5.75, 6.0];

export function formatSwissGrade(grade: number) {
  const fixed = grade.toFixed(2);
  if (fixed.endsWith("00")) return fixed.slice(0, -1);
  if (fixed.endsWith("0")) return fixed.slice(0, -1);
  return fixed;
}
