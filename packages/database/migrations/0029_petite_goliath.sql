CREATE TABLE "agent_runs" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"user_id" uuid NOT NULL,
	"room_id" uuid NOT NULL,
	"session_id" uuid NOT NULL,
	"source_message_id" uuid NOT NULL,
	"status" varchar(30) DEFAULT 'QUEUED' NOT NULL,
	"route" varchar(30),
	"current_step_count" integer DEFAULT 0 NOT NULL,
	"failure_code" varchar(80),
	"expires_at" timestamp with time zone NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"updated_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
CREATE TABLE "agent_steps" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"run_id" uuid NOT NULL,
	"sequence" integer NOT NULL,
	"tool_name" varchar(50) NOT NULL,
	"status" varchar(30) DEFAULT 'QUEUED' NOT NULL,
	"idempotency_key" varchar(128) NOT NULL,
	"input_hash" varchar(64) NOT NULL,
	"input" jsonb NOT NULL,
	"result_metadata" jsonb,
	"failure_code" varchar(80),
	"created_at" timestamp with time zone DEFAULT now() NOT NULL,
	"completed_at" timestamp with time zone
);
--> statement-breakpoint
ALTER TABLE "agent_runs" ADD CONSTRAINT "agent_runs_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "agent_runs" ADD CONSTRAINT "agent_runs_room_id_rooms_id_fk" FOREIGN KEY ("room_id") REFERENCES "public"."rooms"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "agent_runs" ADD CONSTRAINT "agent_runs_session_id_chat_sessions_id_fk" FOREIGN KEY ("session_id") REFERENCES "public"."chat_sessions"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "agent_runs" ADD CONSTRAINT "agent_runs_source_message_id_chat_messages_id_fk" FOREIGN KEY ("source_message_id") REFERENCES "public"."chat_messages"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "agent_steps" ADD CONSTRAINT "agent_steps_run_id_agent_runs_id_fk" FOREIGN KEY ("run_id") REFERENCES "public"."agent_runs"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
CREATE UNIQUE INDEX "agent_runs_source_message_idx" ON "agent_runs" USING btree ("source_message_id");--> statement-breakpoint
CREATE INDEX "agent_runs_status_expiry_idx" ON "agent_runs" USING btree ("status","expires_at");--> statement-breakpoint
CREATE INDEX "agent_runs_session_created_idx" ON "agent_runs" USING btree ("session_id","created_at");--> statement-breakpoint
CREATE UNIQUE INDEX "agent_steps_run_sequence_idx" ON "agent_steps" USING btree ("run_id","sequence");--> statement-breakpoint
CREATE UNIQUE INDEX "agent_steps_idempotency_idx" ON "agent_steps" USING btree ("idempotency_key");--> statement-breakpoint
CREATE INDEX "agent_steps_run_status_idx" ON "agent_steps" USING btree ("run_id","status");