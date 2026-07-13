CREATE TABLE "connection_mutation_receipts" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"actor_scope" varchar(160) NOT NULL,
	"user_id" uuid NOT NULL,
	"actor_device_id" uuid,
	"operation" varchar(40) NOT NULL,
	"aggregate_id" uuid NOT NULL,
	"idempotency_key" varchar(128) NOT NULL,
	"result" jsonb NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
ALTER TABLE "file_browse_requests" ADD COLUMN "query" varchar(100);--> statement-breakpoint
ALTER TABLE "file_browse_requests" ADD COLUMN "search_scope" varchar(30) DEFAULT 'CURRENT_DIRECTORY' NOT NULL;--> statement-breakpoint
ALTER TABLE "room_snapshots" ADD COLUMN "formula_version" varchar(80) DEFAULT 'mousekeeper-cleanliness-v1' NOT NULL;--> statement-breakpoint
ALTER TABLE "connection_mutation_receipts" ADD CONSTRAINT "connection_mutation_receipts_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "connection_mutation_receipts" ADD CONSTRAINT "connection_mutation_receipts_actor_device_id_devices_id_fk" FOREIGN KEY ("actor_device_id") REFERENCES "public"."devices"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
CREATE UNIQUE INDEX "connection_mutation_actor_key_idx" ON "connection_mutation_receipts" USING btree ("actor_scope","idempotency_key");--> statement-breakpoint
CREATE INDEX "connection_mutation_aggregate_idx" ON "connection_mutation_receipts" USING btree ("operation","aggregate_id");