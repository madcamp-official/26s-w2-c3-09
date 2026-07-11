import type { FastifyInstance, FastifyRequest } from 'fastify';

export function registerRequestLogging(app: FastifyInstance) {
  const starts = new WeakMap<FastifyRequest, bigint>();
  app.addHook('onRequest', async (request, reply) => {
    starts.set(request, process.hrtime.bigint());
    void reply.header('x-correlation-id', request.id);
  });
  app.addHook('onResponse', async (request, reply) => {
    const started = starts.get(request);
    const durationMs = started
      ? Number(process.hrtime.bigint() - started) / 1_000_000
      : null;
    console.info(
      JSON.stringify({
        event: 'http.request',
        correlationId: request.id,
        method: request.method,
        route: request.routeOptions.url,
        statusCode: reply.statusCode,
        durationMs: durationMs === null ? null : Math.round(durationMs),
      }),
    );
  });
}
