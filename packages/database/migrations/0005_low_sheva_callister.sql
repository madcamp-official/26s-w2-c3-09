CREATE TABLE "cache_upload_reservations" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"room_id" uuid NOT NULL,
	"desktop_device_id" uuid NOT NULL,
	"source_relative_path" text NOT NULL,
	"source_version" jsonb NOT NULL,
	"source_version_hash" varchar(64) NOT NULL,
	"reserved_bytes" bigint NOT NULL,
	"status" varchar(30) DEFAULT 'RESERVED' NOT NULL,
	"expires_at" timestamp with time zone NOT NULL,
	"object_key" text NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "cached_files" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"room_id" uuid NOT NULL,
	"source_relative_path" text NOT NULL,
	"source_version" jsonb NOT NULL,
	"source_version_hash" varchar(64) NOT NULL,
	"usage_score" integer NOT NULL,
	"manual_pin" boolean DEFAULT false NOT NULL,
	"object_key" text NOT NULL,
	"size_bytes" bigint NOT NULL,
	"availability_status" varchar(30) DEFAULT 'AVAILABLE' NOT NULL,
	"freshness_status" varchar(30) DEFAULT 'VERIFIED_CURRENT' NOT NULL,
	"cached_at" timestamp with time zone DEFAULT now() NOT NULL,
	"last_verified_at" timestamp with time zone NOT NULL,
	"last_accessed_at" timestamp with time zone
);
--> statement-breakpoint
CREATE TABLE "smart_cache_policies" (
	"room_id" uuid PRIMARY KEY NOT NULL,
	"enabled" boolean DEFAULT false NOT NULL,
	"quota_bytes" bigint NOT NULL,
	"max_file_bytes" bigint NOT NULL,
	"excluded_patterns" jsonb DEFAULT '[]'::jsonb NOT NULL,
	"updated_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
ALTER TABLE "cache_upload_reservations" ADD CONSTRAINT "cache_upload_reservations_room_id_rooms_id_fk" FOREIGN KEY ("room_id") REFERENCES "public"."rooms"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "cache_upload_reservations" ADD CONSTRAINT "cache_upload_reservations_desktop_device_id_devices_id_fk" FOREIGN KEY ("desktop_device_id") REFERENCES "public"."devices"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "cached_files" ADD CONSTRAINT "cached_files_room_id_rooms_id_fk" FOREIGN KEY ("room_id") REFERENCES "public"."rooms"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "smart_cache_policies" ADD CONSTRAINT "smart_cache_policies_room_id_rooms_id_fk" FOREIGN KEY ("room_id") REFERENCES "public"."rooms"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
CREATE UNIQUE INDEX "cache_reservation_version_idx" ON "cache_upload_reservations" USING btree ("room_id","source_relative_path","source_version_hash");--> statement-breakpoint
CREATE INDEX "cache_reservation_quota_idx" ON "cache_upload_reservations" USING btree ("room_id","status","expires_at");--> statement-breakpoint
CREATE UNIQUE INDEX "cached_file_version_idx" ON "cached_files" USING btree ("room_id","source_relative_path","source_version_hash");--> statement-breakpoint
CREATE INDEX "cached_files_room_state_idx" ON "cached_files" USING btree ("room_id","availability_status","freshness_status");