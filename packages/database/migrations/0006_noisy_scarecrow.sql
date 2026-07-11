CREATE TABLE "cache_deletion_jobs" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"cached_file_id" uuid NOT NULL,
	"object_key" text NOT NULL,
	"status" varchar(30) DEFAULT 'PENDING' NOT NULL,
	"attempt_count" integer DEFAULT 0 NOT NULL,
	"next_attempt_at" timestamp with time zone DEFAULT now() NOT NULL,
	"last_error_code" varchar(80),
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"completed_at" timestamp with time zone
);
--> statement-breakpoint
CREATE TABLE "cache_reservation_deletion_jobs" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"reservation_id" uuid NOT NULL,
	"object_key" text NOT NULL,
	"status" varchar(30) DEFAULT 'PENDING' NOT NULL,
	"attempt_count" integer DEFAULT 0 NOT NULL,
	"next_attempt_at" timestamp with time zone DEFAULT now() NOT NULL,
	"last_error_code" varchar(80),
	"completed_at" timestamp with time zone
);
--> statement-breakpoint
ALTER TABLE "cache_deletion_jobs" ADD CONSTRAINT "cache_deletion_jobs_cached_file_id_cached_files_id_fk" FOREIGN KEY ("cached_file_id") REFERENCES "public"."cached_files"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "cache_reservation_deletion_jobs" ADD CONSTRAINT "cache_reservation_deletion_jobs_reservation_id_cache_upload_reservations_id_fk" FOREIGN KEY ("reservation_id") REFERENCES "public"."cache_upload_reservations"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
CREATE UNIQUE INDEX "cache_deletion_file_idx" ON "cache_deletion_jobs" USING btree ("cached_file_id");--> statement-breakpoint
CREATE INDEX "cache_deletion_pending_idx" ON "cache_deletion_jobs" USING btree ("status","next_attempt_at");--> statement-breakpoint
CREATE UNIQUE INDEX "cache_reservation_deletion_idx" ON "cache_reservation_deletion_jobs" USING btree ("reservation_id");--> statement-breakpoint
CREATE INDEX "cache_reservation_deletion_pending_idx" ON "cache_reservation_deletion_jobs" USING btree ("status","next_attempt_at");