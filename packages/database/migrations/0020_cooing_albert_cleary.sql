CREATE TABLE "rule_drafts" (
	"id" uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
	"room_id" uuid NOT NULL,
	"created_by_user_id" uuid NOT NULL,
	"name" varchar(120) NOT NULL,
	"definition" jsonb NOT NULL,
	"explanation" text NOT NULL,
	"ambiguities" jsonb DEFAULT '[]'::jsonb NOT NULL,
	"status" varchar(30) DEFAULT 'DRAFT' NOT NULL,
	"rule_id" uuid,
	"confirm_idempotency_key" varchar(128),
	"expires_at" timestamp with time zone NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
ALTER TABLE "rule_drafts" ADD CONSTRAINT "rule_drafts_room_id_rooms_id_fk" FOREIGN KEY ("room_id") REFERENCES "public"."rooms"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "rule_drafts" ADD CONSTRAINT "rule_drafts_created_by_user_id_users_id_fk" FOREIGN KEY ("created_by_user_id") REFERENCES "public"."users"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "rule_drafts" ADD CONSTRAINT "rule_drafts_rule_id_rules_id_fk" FOREIGN KEY ("rule_id") REFERENCES "public"."rules"("id") ON DELETE no action ON UPDATE no action;--> statement-breakpoint
CREATE INDEX "rule_drafts_room_status_idx" ON "rule_drafts" USING btree ("room_id","status");--> statement-breakpoint
CREATE INDEX "rule_drafts_user_status_idx" ON "rule_drafts" USING btree ("created_by_user_id","status");--> statement-breakpoint
CREATE UNIQUE INDEX "rule_drafts_confirm_key_idx" ON "rule_drafts" USING btree ("created_by_user_id","confirm_idempotency_key");