import { render } from 'preact';
import { App } from './ui/App';
import { applyTheme, theme } from './state/store';
import { loadBytes } from './state/files';

applyTheme(theme.value);
window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => applyTheme(theme.value));

render(<App />, document.getElementById('app')!);

// Optional: open tapes given as URL parameters, e.g. ?open=samples/foo.tzx&right=samples/bar.tzx
const params = new URLSearchParams(location.search);
for (const [key, side] of [['open', 0], ['right', 1]] as const) {
  const url = params.get(key);
  if (!url) continue;
  fetch(url)
    .then((r) => (r.ok ? r.arrayBuffer() : Promise.reject(new Error(`${r.status} ${r.statusText}`))))
    .then((buf) => loadBytes(side, decodeURIComponent(url.split('/').pop() ?? url), new Uint8Array(buf)))
    .catch((e) => console.error('Could not open', url, e));
}
