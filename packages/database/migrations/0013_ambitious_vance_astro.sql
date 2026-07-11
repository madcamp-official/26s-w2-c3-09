CREATE TABLE "cache_candidate_batches" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"user_id" uuid NOT NULL,
	"desktop_device_id" uuid NOT NULL,
	"room_id" uuid NOT NULL,
	"idempotency_key" varchar(128) NOT NULL,
	"request_hash" varchar(64) NOT NULL,
	"candidate_count" integer NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
ALTER TABLE "cache_upload_reservations" ADD COLUMN "batch_id" uuid;--> statement-breakpoint
ALTER TABLE "cache_upload_reservations" ADD COLUMN "completion_idempotency_key" varchar(128);--> statement-breakpoint
ALTER TABLE "cache_candidate_batches" ADD CONSTRAINT "cache_candidate_batches_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "cache_candidate_batches" ADD CONSTRAINT "cache_candidate_batches_desktop_device_id_devices_id_fk" FOREIGN KEY ("desktop_device_id") REFERENCES "public"."devices"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "cache_candidate_batches" ADD CONSTRAINT "cache_candidate_batches_room_id_rooms_id_fk" FOREIGN KEY ("room_id") REFERENCES "public"."rooms"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
CREATE UNIQUE INDEX "cache_candidate_batches_user_key_idx" ON "cache_candidate_batches" USING btree ("user_id","idempotency_key");--> statement-breakpoint
ALTER TABLE "cache_upload_reservations" ADD CONSTRAINT "cache_upload_reservations_batch_id_cache_candidate_batches_id_fk" FOREIGN KEY ("batch_id") REFERENCES "public"."cache_candidate_batches"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
CREATE UNIQUE INDEX "cache_reservation_completion_key_idx" ON "cache_upload_reservations" USING btree ("desktop_device_id","completion_idempotency_key");--> statement-breakpoint
CREATE INDEX "cache_reservation_batch_idx" ON "cache_upload_reservations" USING btree ("batch_id");