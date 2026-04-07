/**
 * Export chart data as CSV file download
 */
export function exportCSV(data: Record<string, unknown>[], filename: string) {
  if (!data || data.length === 0) return;

  const headers = Object.keys(data[0]);
  const csvRows = [
    headers.join(','),
    ...data.map(row =>
      headers.map(h => {
        const val = row[h];
        const str = String(val ?? '');
        return str.includes(',') || str.includes('"') || str.includes('\n')
          ? `"${str.replace(/"/g, '""')}"`
          : str;
      }).join(',')
    )
  ];

  const blob = new Blob([csvRows.join('\n')], { type: 'text/csv;charset=utf-8;' });
  downloadBlob(blob, `${filename}.csv`);
}

/**
 * Export a DOM element (chart container) as PNG image
 */
export function exportChartImage(elementId: string, filename: string) {
  const el = document.getElementById(elementId);
  if (!el) return;

  // Use canvas approach — clone the element and render to canvas
  // Since we can't use html2canvas without a dependency, we'll use SVG foreignObject
  const rect = el.getBoundingClientRect();
  const width = rect.width * 2; // 2x for retina
  const height = rect.height * 2;

  const svgData = new XMLSerializer().serializeToString(el);
  const svgBlob = new Blob([svgData], { type: 'image/svg+xml;charset=utf-8' });
  const url = URL.createObjectURL(svgBlob);

  const img = new Image();
  img.onload = () => {
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.scale(2, 2);
    // Draw background
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--color-surface').trim() || '#111827';
    ctx.fillRect(0, 0, rect.width, rect.height);
    ctx.drawImage(img, 0, 0, rect.width, rect.height);
    canvas.toBlob(blob => {
      if (blob) downloadBlob(blob, `${filename}.png`);
      URL.revokeObjectURL(url);
    }, 'image/png');
  };
  img.src = url;
}

function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
