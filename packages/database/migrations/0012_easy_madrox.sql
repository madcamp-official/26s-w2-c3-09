ALTER TABLE "executions" ADD COLUMN "result_idempotency_key" varchar(128);--> statement-breakpoint
ALTER TABLE "proposals" ADD COLUMN "idempotency_key" varchar(128);--> statement-breakpoint
CREATE UNIQUE INDEX "executions_result_idempotency_idx" ON "executions" USING btree ("result_idempotency_key");--> statement-breakpoint
CREATE UNIQUE INDEX "proposals_idempotency_idx" ON "proposals" USING btree ("idempotency_key");