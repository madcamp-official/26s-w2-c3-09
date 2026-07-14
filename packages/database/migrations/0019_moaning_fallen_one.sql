CREATE TABLE "chat_sessions" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"user_id" uuid NOT NULL,
	"room_id" uuid NOT NULL,
	"title" varchar(120) NOT NULL,
	"summary" text,
	"status" varchar(30) DEFAULT 'ACTIVE' NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"updated_at" timestamp with time zone DEFAULT now() NOT NULL,
	"deleted_at" timestamp with time zone
);
--> statement-breakpoint
CREATE TABLE "command_drafts" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"session_id" uuid NOT NULL,
	"source_message_id" uuid NOT NULL,
	"room_id" uuid NOT NULL,
	"created_by_user_id" uuid NOT NULL,
	"intent" varchar(40) NOT NULL,
	"arguments" jsonb NOT NULL,
	"confirmation_summary" text NOT NULL,
	"status" varchar(30) DEFAULT 'DRAFT' NOT NULL,
	"command_id" uuid,
	"expires_at" timestamp with time zone NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
ALTER TABLE "chat_messages" ADD COLUMN "session_id" uuid;--> statement-breakpoint
ALTER TABLE "chat_messages" ADD COLUMN "message_type" varchar(30) DEFAULT 'TEXT' NOT NULL;--> statement-breakpoint
ALTER TABLE "chat_messages" ADD COLUMN "structured_payload" jsonb;--> statement-breakpoint
ALTER TABLE "chat_sessions" ADD CONSTRAINT "chat_sessions_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chat_sessions" ADD CONSTRAINT "chat_sessions_room_id_rooms_id_fk" FOREIGN KEY ("room_id") REFERENCES "public"."rooms"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "command_drafts" ADD CONSTRAINT "command_drafts_session_id_chat_sessions_id_fk" FOREIGN KEY ("session_id") REFERENCES "public"."chat_sessions"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "command_drafts" ADD CONSTRAINT "command_drafts_source_message_id_chat_messages_id_fk" FOREIGN KEY ("source_message_id") REFERENCES "public"."chat_messages"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "command_drafts" ADD CONSTRAINT "command_drafts_room_id_rooms_id_fk" FOREIGN KEY ("room_id") REFERENCES "public"."rooms"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "command_drafts" ADD CONSTRAINT "command_drafts_created_by_user_id_users_id_fk" FOREIGN KEY ("created_by_user_id") REFERENCES "public"."users"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "command_drafts" ADD CONSTRAINT "command_drafts_command_id_commands_id_fk" FOREIGN KEY ("command_id") REFERENCES "public"."commands"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
CREATE INDEX "chat_sessions_user_room_status_updated_idx" ON "chat_sessions" USING btree ("user_id","room_id","status","updated_at");--> statement-breakpoint
CREATE INDEX "command_drafts_session_status_idx" ON "command_drafts" USING btree ("session_id","status");--> statement-breakpoint
CREATE INDEX "command_drafts_room_status_idx" ON "command_drafts" USING btree ("room_id","status");--> statement-breakpoint
CREATE INDEX "command_drafts_user_status_idx" ON "command_drafts" USING btree ("created_by_user_id","status");--> statement-breakpoint
ALTER TABLE "chat_messages" ADD CONSTRAINT "chat_messages_session_id_chat_sessions_id_fk" FOREIGN KEY ("session_id") REFERENCES "public"."chat_sessions"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
CREATE INDEX "chat_session_time_idx" ON "chat_messages" USING btree ("session_id","created_at","id");