CREATE TABLE "notification_jobs" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"user_id" uuid NOT NULL,
	"sync_event_id" uuid NOT NULL,
	"event_type" varchar(100) NOT NULL,
	"title" varchar(120) NOT NULL,
	"body" varchar(500) NOT NULL,
	"status" varchar(30) DEFAULT 'PENDING' NOT NULL,
	"attempt_count" integer DEFAULT 0 NOT NULL,
	"next_attempt_at" timestamp with time zone DEFAULT now() NOT NULL,
	"last_error_code" varchar(80),
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"completed_at" timestamp with time zone
);
--> statement-breakpoint
CREATE TABLE "push_notification_tokens" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"user_id" uuid NOT NULL,
	"token" text NOT NULL,
	"token_hash" varchar(64) NOT NULL,
	"platform" "platform" NOT NULL,
	"status" varchar(30) DEFAULT 'ACTIVE' NOT NULL,
	"last_seen_at" timestamp with time zone DEFAULT now() NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"revoked_at" timestamp with time zone
);
--> statement-breakpoint
ALTER TABLE "notification_jobs" ADD CONSTRAINT "notification_jobs_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "notification_jobs" ADD CONSTRAINT "notification_jobs_sync_event_id_sync_events_id_fk" FOREIGN KEY ("sync_event_id") REFERENCES "public"."sync_events"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "push_notification_tokens" ADD CONSTRAINT "push_notification_tokens_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
CREATE UNIQUE INDEX "notification_jobs_sync_event_idx" ON "notification_jobs" USING btree ("sync_event_id");--> statement-breakpoint
CREATE INDEX "notification_jobs_pending_idx" ON "notification_jobs" USING btree ("status","next_attempt_at");--> statement-breakpoint
CREATE INDEX "notification_jobs_user_status_idx" ON "notification_jobs" USING btree ("user_id","status");--> statement-breakpoint
CREATE UNIQUE INDEX "push_notification_tokens_hash_idx" ON "push_notification_tokens" USING btree ("token_hash");--> statement-breakpoint
CREATE INDEX "push_notification_tokens_user_status_idx" ON "push_notification_tokens" USING btree ("user_id","status");