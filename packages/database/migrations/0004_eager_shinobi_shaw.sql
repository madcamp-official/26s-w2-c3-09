CREATE TABLE "affinity_events" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"character_profile_id" uuid NOT NULL,
	"event_type" varchar(80) NOT NULL,
	"delta" integer NOT NULL,
	"source_decision_id" uuid,
	"source_execution_id" uuid,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "character_profiles" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"user_id" uuid NOT NULL,
	"appearance" jsonb DEFAULT '{}'::jsonb NOT NULL,
	"room_theme" varchar(80),
	"affinity_total" integer DEFAULT 0 NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"updated_at" timestamp with time zone DEFAULT now() NOT NULL,
	CONSTRAINT "character_profiles_user_id_unique" UNIQUE("user_id")
);
--> statement-breakpoint
CREATE TABLE "chat_messages" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"room_id" uuid NOT NULL,
	"sender_type" varchar(30) NOT NULL,
	"content" text NOT NULL,
	"command_id" uuid,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
ALTER TABLE "affinity_events" ADD CONSTRAINT "affinity_events_character_profile_id_character_profiles_id_fk" FOREIGN KEY ("character_profile_id") REFERENCES "public"."character_profiles"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "affinity_events" ADD CONSTRAINT "affinity_events_source_decision_id_decisions_id_fk" FOREIGN KEY ("source_decision_id") REFERENCES "public"."decisions"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "affinity_events" ADD CONSTRAINT "affinity_events_source_execution_id_executions_id_fk" FOREIGN KEY ("source_execution_id") REFERENCES "public"."executions"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "character_profiles" ADD CONSTRAINT "character_profiles_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chat_messages" ADD CONSTRAINT "chat_messages_room_id_rooms_id_fk" FOREIGN KEY ("room_id") REFERENCES "public"."rooms"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "chat_messages" ADD CONSTRAINT "chat_messages_command_id_commands_id_fk" FOREIGN KEY ("command_id") REFERENCES "public"."commands"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
CREATE UNIQUE INDEX "affinity_decision_event_idx" ON "affinity_events" USING btree ("source_decision_id","event_type");--> statement-breakpoint
CREATE UNIQUE INDEX "affinity_execution_event_idx" ON "affinity_events" USING btree ("source_execution_id","event_type");--> statement-breakpoint
CREATE INDEX "chat_room_time_idx" ON "chat_messages" USING btree ("room_id","created_at");