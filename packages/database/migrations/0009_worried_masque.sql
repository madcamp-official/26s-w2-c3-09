ALTER TABLE "sync_events" ADD COLUMN "schema_version" integer DEFAULT 1 NOT NULL;--> statement-breakpoint
ALTER TABLE "sync_events" ADD COLUMN "correlation_id" uuid DEFAULT gen_random_uuid() NOT NULL;