import { describe, expect, it } from 'vitest';

import { shotsDirOf, workerTmpDir } from './tmp.ts';

const RUN = '20261117-162121-9d0678';

describe('workerTmpDir', () => {
  it('takes the folder rBrowser names, with backslashes on Windows', () => {
    expect(workerTmpDir(`C:/Users/me/AppData/Local/rBrowser/flows-tmp/${RUN}`, RUN, {}, 'win32', 'C:\\T')).toBe(
      `C:\\Users\\me\\AppData\\Local\\rBrowser\\flows-tmp\\${RUN}`,
    );
    expect(workerTmpDir(`/home/me/.local/state/rBrowser/flows-tmp/${RUN}`, RUN, {}, 'linux', '/tmp')).toBe(
      `/home/me/.local/state/rBrowser/flows-tmp/${RUN}`,
    );
  });

  it("falls back on Windows to the app's local folder, never the project's share", () => {
    const env = { LOCALAPPDATA: 'C:\\Users\\me\\AppData\\Local' };
    const dir = workerTmpDir(null, RUN, env, 'win32', 'C:\\T');
    expect(dir).toBe(`C:\\Users\\me\\AppData\\Local\\rBrowser\\flows-tmp\\${RUN}`);
    expect(dir.startsWith('\\\\')).toBe(false);
  });

  it('falls back to the system temp folder without LOCALAPPDATA and off Windows', () => {
    expect(workerTmpDir(null, RUN, { LOCALAPPDATA: '' }, 'win32', 'C:\\Users\\me\\AppData\\Local\\Temp')).toBe(
      `C:\\Users\\me\\AppData\\Local\\Temp\\rbrowser-flows-tmp\\${RUN}`,
    );
    expect(workerTmpDir(null, RUN, { LOCALAPPDATA: 'C:\\x' }, 'linux', '/tmp')).toBe(`/tmp/rbrowser-flows-tmp/${RUN}`);
  });
});

describe('shotsDirOf', () => {
  it('keeps the screenshots apart from the temp files, with the separator of the platform', () => {
    expect(shotsDirOf(`/tmp/rbrowser-flows-tmp/${RUN}`, 'linux')).toBe(`/tmp/rbrowser-flows-tmp/${RUN}/out`);
    expect(shotsDirOf(`C:\\L\\rBrowser\\flows-tmp\\${RUN}`, 'win32')).toBe(`C:\\L\\rBrowser\\flows-tmp\\${RUN}\\out`);
  });
});
