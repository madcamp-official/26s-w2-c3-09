import Fastify from 'fastify';
import { registerRequestLogging } from './request-logging';

describe('registerRequestLogging', () => {
  it('logs the route template and correlation id without path or query values', async () => {
    const app = Fastify();
    const info = jest.spyOn(console, 'info').mockImplementation(() => {});
    registerRequestLogging(app);
    app.get('/files/:name', async () => ({ ok: true }));
    const response = await app.inject({
      method: 'GET',
      url: '/files/private.pdf?token=secret',
    });

    expect(response.statusCode).toBe(200);
    expect(response.headers['x-correlation-id']).toBeTruthy();
    const log = info.mock.calls.at(-1)?.[0] as string;
    expect(JSON.parse(log)).toMatchObject({
      event: 'http.request',
      method: 'GET',
      route: '/files/:name',
      statusCode: 200,
    });
    expect(log).not.toContain('private.pdf');
    expect(log).not.toContain('secret');

    info.mockRestore();
    await app.close();
  });
});
