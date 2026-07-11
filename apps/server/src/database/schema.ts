import { index, jsonb, pgEnum, pgTable, timestamp, uniqueIndex, uuid, varchar } from 'drizzle-orm/pg-core';

export const platform = pgEnum('platform', ['WINDOWS', 'MACOS', 'ANDROID', 'IOS']);
export const deviceStatus = pgEnum('device_status', ['ACTIVE', 'REVOKED']);
export const roomStatus = pgEnum('room_status', ['ACTIVE', 'PAUSED', 'REMOVED']);
export const commandStatus = pgEnum('command_status', [
  'QUEUED', 'DELIVERED', 'ANALYZING', 'PROPOSAL_READY', 'WAITING_APPROVAL',
  'APPROVED', 'REJECTED', 'EXPIRED', 'EXECUTING', 'SUCCEEDED',
  'PARTIALLY_SUCCEEDED', 'FAILED', 'STALE',
]);

export const users = pgTable('users', {
  id: uuid('id').primaryKey().defaultRandom(),
  authProviderUid: varchar('auth_provider_uid', { length: 128 }).notNull().unique(),
  displayName: varchar('display_name', { length: 120 }).notNull(),
  createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
});

export const devices = pgTable('devices', {
  id: uuid('id').primaryKey().defaultRandom(),
  userId: uuid('user_id').notNull().references(() => users.id),
  platform: platform('platform').notNull(),
  deviceName: varchar('device_name', { length: 120 }).notNull(),
  status: deviceStatus('status').notNull().default('ACTIVE'),
  lastSeenAt: timestamp('last_seen_at', { withTimezone: true }),
  createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
}, (t) => [index('devices_user_status_idx').on(t.userId, t.status)]);

export const rooms = pgTable('rooms', {
  id: uuid('id').primaryKey().defaultRandom(),
  userId: uuid('user_id').notNull().references(() => users.id),
  desktopDeviceId: uuid('desktop_device_id').notNull().references(() => devices.id),
  name: varchar('name', { length: 120 }).notNull(),
  rootAlias: varchar('root_alias', { length: 120 }).notNull(),
  status: roomStatus('status').notNull().default('ACTIVE'),
  createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
});

export const commands = pgTable('commands', {
  id: uuid('id').primaryKey().defaultRandom(),
  roomId: uuid('room_id').notNull().references(() => rooms.id),
  targetDeviceId: uuid('target_device_id').notNull().references(() => devices.id),
  createdByUserId: uuid('created_by_user_id').notNull().references(() => users.id),
  intent: varchar('intent', { length: 40 }).notNull(),
  payload: jsonb('payload').notNull(),
  status: commandStatus('status').notNull().default('QUEUED'),
  idempotencyKey: varchar('idempotency_key', { length: 128 }).notNull(),
  createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
}, (t) => [
  uniqueIndex('commands_user_idempotency_idx').on(t.createdByUserId, t.idempotencyKey),
  index('commands_device_status_created_idx').on(t.targetDeviceId, t.status, t.createdAt),
]);
