import {
  cachedFiles,
  fileBrowseRequests,
  proposalItems,
  proposals,
  roomSnapshots,
  rooms,
  rules,
  type Database,
} from '@mousekeeper/database';
import { and, asc, desc, eq, ne } from 'drizzle-orm';
import type { FileContext, RoomContext } from './ai.provider';

type Transaction = Parameters<Parameters<Database['transaction']>[0]>[0];
type DbExecutor = Database | Transaction;

export async function buildRoomContext(
  db: DbExecutor,
  roomId: string,
): Promise<RoomContext | null> {
  const room = (
    await db.select().from(rooms).where(eq(rooms.id, roomId)).limit(1)
  )[0];
  if (!room) return null;
  const roomRules = await db
    .select({ name: rules.name, definition: rules.definition })
    .from(rules)
    .where(and(eq(rules.roomId, roomId), eq(rules.enabled, true)))
    .orderBy(asc(rules.priority))
    .limit(20);
  return {
    roomName: room.name,
    rootAlias: room.rootAlias,
    existingRules: roomRules.map((rule) => ({
      name: rule.name,
      destinationTemplate: destinationTemplateFromDefinition(rule.definition),
    })),
  };
}

export async function buildFileContext(
  db: DbExecutor,
  roomId: string,
): Promise<FileContext> {
  const [files, latestBrowse, latestSnapshot, proposalRows] =
    await Promise.all([
      db
        .select({
          sourceRelativePath: cachedFiles.sourceRelativePath,
          sourceVersion: cachedFiles.sourceVersion,
          sizeBytes: cachedFiles.sizeBytes,
          cachedAt: cachedFiles.cachedAt,
        })
        .from(cachedFiles)
        .where(
          and(
            eq(cachedFiles.roomId, roomId),
            eq(cachedFiles.availabilityStatus, 'AVAILABLE'),
            ne(cachedFiles.freshnessStatus, 'STALE'),
          ),
        )
        .orderBy(desc(cachedFiles.cachedAt))
        .limit(200),
      db
        .select({
          relativeDirectory: fileBrowseRequests.relativeDirectory,
          status: fileBrowseRequests.status,
          resultPage: fileBrowseRequests.resultPage,
          createdAt: fileBrowseRequests.createdAt,
        })
        .from(fileBrowseRequests)
        .where(eq(fileBrowseRequests.roomId, roomId))
        .orderBy(desc(fileBrowseRequests.createdAt))
        .limit(1),
      db
        .select({
          score: roomSnapshots.score,
          metrics: roomSnapshots.metrics,
          calculatedAt: roomSnapshots.calculatedAt,
        })
        .from(roomSnapshots)
        .where(eq(roomSnapshots.roomId, roomId))
        .orderBy(desc(roomSnapshots.calculatedAt))
        .limit(1),
      db
        .select({
          proposalId: proposals.id,
          status: proposals.status,
          summary: proposals.summary,
          createdAt: proposals.createdAt,
          actionType: proposalItems.actionType,
          sourceRelativePath: proposalItems.sourceRelativePath,
          destinationRelativePath: proposalItems.destinationRelativePath,
          itemOrder: proposalItems.itemOrder,
        })
        .from(proposals)
        .leftJoin(proposalItems, eq(proposalItems.proposalId, proposals.id))
        .where(eq(proposals.roomId, roomId))
        .orderBy(desc(proposals.createdAt), asc(proposalItems.itemOrder))
        .limit(30),
    ]);

  const knownFolders = new Set<string>();
  const topLevelFolders = new Set<string>();
  const extensionCounts = new Map<string, number>();
  for (const file of files) {
    const path = file.sourceRelativePath;
    for (const folder of folderPrefixes(path)) knownFolders.add(folder);
    const top = topLevelFolder(path);
    if (top) topLevelFolders.add(top);
    const extension = fileExtension(path);
    extensionCounts.set(extension, (extensionCounts.get(extension) ?? 0) + 1);
  }

  const browse = latestBrowse[0] ?? null;
  const browseEntries = browseEntriesFromResultPage(browse?.resultPage);
  for (const entry of browseEntries) {
    if (entry.type !== 'DIRECTORY') continue;
    knownFolders.add(entry.relativePath);
    const top = topLevelFolder(entry.relativePath);
    if (top) topLevelFolders.add(top);
  }

  return {
    source: 'SERVER_CACHE',
    isLiveFilesystemSnapshot: false,
    generatedAt: new Date().toISOString(),
    topLevelFolders: [...topLevelFolders].sort().slice(0, 30),
    knownFolders: [...knownFolders].sort().slice(0, 50),
    extensionDistribution: [...extensionCounts.entries()]
      .map(([extension, count]) => ({ extension, count }))
      .sort((a, b) => b.count - a.count || a.extension.localeCompare(b.extension))
      .slice(0, 20),
    recentFiles: files.slice(0, 20).map((file) => ({
      relativePath: file.sourceRelativePath,
      extension: fileExtension(file.sourceRelativePath),
      sizeBytes: file.sizeBytes,
      modifiedAt: modifiedAtFromSourceVersion(file.sourceVersion),
      cachedAt: file.cachedAt.toISOString(),
    })),
    latestBrowse: browse
      ? {
          relativeDirectory: browse.relativeDirectory,
          status: browse.status,
          requestedAt: browse.createdAt.toISOString(),
          directories: browseEntries
            .filter((entry) => entry.type === 'DIRECTORY')
            .map((entry) => entry.relativePath)
            .slice(0, 30),
          files: browseEntries
            .filter((entry) => entry.type === 'FILE')
            .map((entry) => entry.relativePath)
            .slice(0, 30),
        }
      : null,
    latestSnapshot: latestSnapshot[0]
      ? {
          score: latestSnapshot[0].score,
          metrics: latestSnapshot[0].metrics,
          calculatedAt: latestSnapshot[0].calculatedAt.toISOString(),
        }
      : null,
    recentProposals: summarizeProposals(proposalRows),
  };
}

function destinationTemplateFromDefinition(value: unknown): string | null {
  if (value == null || typeof value !== 'object') return null;
  const action = (value as Record<string, unknown>).action;
  if (action == null || typeof action !== 'object') return null;
  const destination = (action as Record<string, unknown>).destinationTemplate;
  return typeof destination === 'string' ? destination : null;
}

function fileExtension(path: string) {
  const name = path.split('/').pop() ?? path;
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot).toLowerCase() : '';
}

function topLevelFolder(path: string) {
  const [first, second] = path.split('/');
  return first && second ? first : null;
}

function folderPrefixes(path: string) {
  const parts = path.split('/').filter(Boolean);
  const folders = parts.slice(0, -1);
  return folders.map((_, index) => folders.slice(0, index + 1).join('/'));
}

function modifiedAtFromSourceVersion(value: unknown) {
  if (!value || typeof value !== 'object') return null;
  const record = value as Record<string, unknown>;
  if (typeof record.modifiedAt === 'string') return record.modifiedAt;
  if (typeof record.mtimeMs === 'number' && Number.isFinite(record.mtimeMs)) {
    return new Date(record.mtimeMs).toISOString();
  }
  return null;
}

function browseEntriesFromResultPage(value: unknown) {
  if (!value || typeof value !== 'object') return [];
  const entries = (value as Record<string, unknown>).entries;
  if (!Array.isArray(entries)) return [];
  return entries
    .map((entry) => {
      if (!entry || typeof entry !== 'object') return null;
      const record = entry as Record<string, unknown>;
      if (
        typeof record.relativePath !== 'string' ||
        (record.type !== 'FILE' && record.type !== 'DIRECTORY')
      ) {
        return null;
      }
      return {
        relativePath: record.relativePath,
        type: record.type,
      };
    })
    .filter((entry): entry is { relativePath: string; type: 'FILE' | 'DIRECTORY' } =>
      entry !== null,
    );
}

function summarizeProposals(
  rows: {
    proposalId: string;
    status: string;
    summary: unknown;
    createdAt: Date;
    actionType: string | null;
    sourceRelativePath: string | null;
    destinationRelativePath: string | null;
  }[],
) {
  const byId = new Map<
    string,
    {
      status: string;
      summary: unknown;
      createdAt: string;
      itemCount: number;
      sampleItems: {
        actionType: string;
        sourceRelativePath: string | null;
        destinationRelativePath: string | null;
      }[];
    }
  >();
  for (const row of rows) {
    let proposal = byId.get(row.proposalId);
    if (!proposal) {
      proposal = {
        status: row.status,
        summary: row.summary,
        createdAt: row.createdAt.toISOString(),
        itemCount: 0,
        sampleItems: [],
      };
      byId.set(row.proposalId, proposal);
    }
    if (!row.actionType) continue;
    proposal.itemCount += 1;
    if (proposal.sampleItems.length < 5) {
      proposal.sampleItems.push({
        actionType: row.actionType,
        sourceRelativePath: row.sourceRelativePath,
        destinationRelativePath: row.destinationRelativePath,
      });
    }
  }
  return [...byId.values()].slice(0, 5);
}
