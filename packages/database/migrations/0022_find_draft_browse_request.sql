ALTER TABLE "command_drafts" ADD COLUMN "file_browse_request_id" uuid;--> statement-breakpoint
ALTER TABLE "command_drafts" ADD COLUMN "confirm_idempotency_key" varchar(128);--> statement-breakpoint
ALTER TABLE "file_browse_requests" ADD COLUMN "extensions" jsonb DEFAULT '[]'::jsonb NOT NULL;--> statement-breakpoint
ALTER TABLE "file_browse_requests" ADD COLUMN "limit" integer DEFAULT 200 NOT NULL;--> statement-breakpoint
ALTER TABLE "command_drafts" ADD CONSTRAINT "command_drafts_file_browse_request_id_file_browse_requests_id_fk" FOREIGN KEY ("file_browse_request_id") REFERENCES "public"."file_browse_requests"("id") ON DELETE no action ON UPDATE no action;