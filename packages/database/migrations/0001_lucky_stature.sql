CREATE TABLE "file_browse_requests" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"room_id" uuid NOT NULL,
	"desktop_device_id" uuid NOT NULL,
	"relative_directory" text NOT NULL,
	"cursor" varchar(512),
	"status" varchar(40) DEFAULT 'REQUESTED' NOT NULL,
	"failure_code" varchar(60),
	"result_page" jsonb,
	"desktop_generation" varchar(128),
	"expires_at" timestamp with time zone NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "file_transfers" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"room_id" uuid NOT NULL,
	"desktop_device_id" uuid NOT NULL,
	"requested_by_user_id" uuid NOT NULL,
	"source_relative_path" text NOT NULL,
	"source_version" jsonb,
	"status" varchar(40) DEFAULT 'REQUESTED' NOT NULL,
	"object_key" text,
	"size_bytes" bigint,
	"sha256" varchar(64),
	"expires_at" timestamp with time zone NOT NULL,
	"completed_at" timestamp with time zone,
	"idempotency_key" varchar(128) NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "object_deletion_jobs" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"transfer_id" uuid NOT NULL,
	"object_key" text NOT NULL,
	"status" varchar(30) DEFAULT 'PENDING' NOT NULL,
	"attempt_count" integer DEFAULT 0 NOT NULL,
	"next_attempt_at" timestamp with time zone DEFAULT now() NOT NULL,
	"last_error_code" varchar(80),
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"completed_at" timestamp with time zone
);
--> statement-breakpoint
ALTER TABLE "file_browse_requests" ADD CONSTRAINT "file_browse_requests_room_id_rooms_id_fk" FOREIGN KEY ("room_id") REFERENCES "public"."rooms"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "file_browse_requests" ADD CONSTRAINT "file_browse_requests_desktop_device_id_devices_id_fk" FOREIGN KEY ("desktop_device_id") REFERENCES "public"."devices"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "file_transfers" ADD CONSTRAINT "file_transfers_room_id_rooms_id_fk" FOREIGN KEY ("room_id") REFERENCES "public"."rooms"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "file_transfers" ADD CONSTRAINT "file_transfers_desktop_device_id_devices_id_fk" FOREIGN KEY ("desktop_device_id") REFERENCES "public"."devices"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "file_transfers" ADD CONSTRAINT "file_transfers_requested_by_user_id_users_id_fk" FOREIGN KEY ("requested_by_user_id") REFERENCES "public"."users"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "object_deletion_jobs" ADD CONSTRAINT "object_deletion_jobs_transfer_id_file_transfers_id_fk" FOREIGN KEY ("transfer_id") REFERENCES "public"."file_transfers"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
CREATE INDEX "browse_device_status_idx" ON "file_browse_requests" USING btree ("desktop_device_id","status","created_at");--> statement-breakpoint
CREATE UNIQUE INDEX "transfers_user_idempotency_idx" ON "file_transfers" USING btree ("requested_by_user_id","idempotency_key");--> statement-breakpoint
CREATE INDEX "transfers_device_status_idx" ON "file_transfers" USING btree ("desktop_device_id","status","created_at");--> statement-breakpoint
CREATE INDEX "transfers_expiry_idx" ON "file_transfers" USING btree ("expires_at","status");--> statement-breakpoint
CREATE INDEX "deletion_jobs_pending_idx" ON "object_deletion_jobs" USING btree ("status","next_attempt_at");