import { loadSimulatorConfig } from './config';
describe('simulator config', () => {
  it('does not start without real API credentials', () => { expect(() => loadSimulatorConfig({})).toThrow('UNCONFIGURED'); });
});
