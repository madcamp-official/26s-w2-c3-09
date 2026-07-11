import { loadEnvironment } from './environment';

describe('loadEnvironment', () => {
  it('fails fast when required values are absent', () => {
    expect(() => loadEnvironment({})).toThrow('UNCONFIGURED');
  });
});
