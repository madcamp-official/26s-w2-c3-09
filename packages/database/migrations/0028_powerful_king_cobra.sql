ALTER TABLE "proposals" ADD COLUMN "session_id" uuid;--> statement-breakpoint
ALTER TABLE "proposals" ADD COLUMN "chat_message_id" uuid;--> statement-breakpoint
ALTER TABLE "proposals" ADD CONSTRAINT "proposals_session_id_chat_sessions_id_fk" FOREIGN KEY ("session_id") REFERENCES "public"."chat_sessions"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "proposals" ADD CONSTRAINT "proposals_chat_message_id_chat_messages_id_fk" FOREIGN KEY ("chat_message_id") REFERENCES "public"."chat_messages"("id") ON DELETE no action ON UPDATE no action;
