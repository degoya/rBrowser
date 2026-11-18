// The form autofill plugin's files in the test's window: the fixture form as a view's document
// and the page script as `evaluate` runs it; the loader is plugins/web-common/src/load.ts.
import { pluginFile } from '../../../web-common/src/load';

export { pluginFile, writePluginFile } from '../../../web-common/src/load';

/** The page script with `request` and the agent's `targetOf`, exactly as the component sends it (src/fill.rs `script`). */
// eslint-disable-next-line @typescript-eslint/no-explicit-any -- what the page script answers is the test's to check
export const page = (request: Record<string, unknown>): any =>
  (0, eval)(`(\n${pluginFile('src/page.js')}\n)(${JSON.stringify(request)}, (\n${pluginFile('src/target.js')}\n)())`);

/** The fixture form as the document, without its script; returns its event log. */
export function loadForm(): { events: { id: string; type: string; trusted: unknown }[]; submits: () => number } {
  const html = pluginFile('tests/fixtures/forms/register.html');
  document.documentElement.setAttribute('lang', 'en');
  document.head.innerHTML = `<style>${/<style>([\s\S]*?)<\/style>/.exec(html)?.[1] ?? ''}</style>`;
  document.body.innerHTML = (/<body>([\s\S]*)<\/body>/.exec(html)?.[1] ?? '').replace(/<script>[\s\S]*?<\/script>/g, '');
  const events: { id: string; type: string; trusted: unknown }[] = [];
  let submits = 0;
  const form = document.getElementById('register') as HTMLFormElement;
  for (const type of ['input', 'change']) {
    form.addEventListener(type, (event) => {
      const target = event.target as HTMLInputElement;
      events.push({ id: target.id || target.name, type, trusted: event.isTrusted });
    });
  }
  form.addEventListener('submit', () => {
    submits += 1;
  });
  return { events, submits: () => submits };
}
