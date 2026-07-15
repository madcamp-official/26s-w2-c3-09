ALTER TABLE "smart_cache_policies"
ALTER COLUMN "enabled" SET DEFAULT true;

UPDATE "smart_cache_policies"
SET "enabled" = true,
    "updated_at" = now()
WHERE "enabled" = false;
