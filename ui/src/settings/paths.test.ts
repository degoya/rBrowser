import { describe, expect, it } from 'vitest';
import { projectRelative } from './paths';

describe('a picked path of a project key (RB-080-20)', () => {
  it('becomes relative to the project with forward slashes', () => {
    expect(projectRelative('C:\\dev\\site\\.rbrowser\\out\\shots', 'C:\\dev\\site')).toBe('.rbrowser/out/shots');
    expect(projectRelative('c:\\DEV\\site\\mocks\\', 'C:/dev/site/')).toBe('mocks');
  });

  it('stays as chosen outside the project or without one', () => {
    expect(projectRelative('D:\\shots', 'C:\\dev\\site')).toBe('D:\\shots');
    expect(projectRelative('C:\\dev\\site-two\\x', 'C:\\dev\\site')).toBe('C:\\dev\\site-two\\x');
    expect(projectRelative('C:\\dev\\site', 'C:\\dev\\site')).toBe('C:\\dev\\site');
    expect(projectRelative('C:\\x', null)).toBe('C:\\x');
  });
});
