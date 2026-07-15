ALTER TABLE "agent_runs" ADD COLUMN "resume_context" jsonb;--> statement-breakpoint
ALTER TABLE "agent_steps" ADD COLUMN "external_request_id" uuid;--> statement-breakpoint
CREATE INDEX "agent_steps_external_request_idx" ON "agent_steps" USING btree ("external_request_id");