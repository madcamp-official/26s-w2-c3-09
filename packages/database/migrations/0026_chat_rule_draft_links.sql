ALTER TABLE "rule_drafts" ADD COLUMN "session_id" uuid;
--> statement-breakpoint
ALTER TABLE "rule_drafts" ADD COLUMN "source_message_id" uuid;
--> statement-breakpoint
ALTER TABLE "rule_drafts" ADD CONSTRAINT "rule_drafts_session_id_chat_sessions_id_fk" FOREIGN KEY ("session_id") REFERENCES "public"."chat_sessions"("id") ON DELETE no action ON UPDATE no action;
--> statement-breakpoint
ALTER TABLE "rule_drafts" ADD CONSTRAINT "rule_drafts_source_message_id_chat_messages_id_fk" FOREIGN KEY ("source_message_id") REFERENCES "public"."chat_messages"("id") ON DELETE no action ON UPDATE no action;
--> statement-breakpoint
CREATE INDEX "rule_drafts_session_status_idx" ON "rule_drafts" USING btree ("session_id","status");
